use async_trait::async_trait;
use matrix_sdk::{
    room::Room,
    ruma::{
        events::{
            presence::PresenceEvent,
            reaction::ReactionEventContent,
            room::{
                aliases::AliasesEventContent,
                avatar::AvatarEventContent,
                canonical_alias::CanonicalAliasEventContent,
                join_rules::JoinRulesEventContent,
                member::MemberEventContent,
                message::{feedback::FeedbackEventContent, MessageEventContent},
                name::NameEventContent,
                power_levels::PowerLevelsEventContent,
                redaction::SyncRedactionEvent,
                tombstone::TombstoneEventContent,
            },
            AnyMessageEvent, AnyRoomEvent, AnySyncMessageEvent, AnySyncRoomEvent,
            StrippedStateEvent, SyncMessageEvent, SyncStateEvent,
        },
        RoomId, UInt,
    },
};

use crate::{events::*, state};

pub fn send_read_receipt_current_room(client: matrix_sdk::Client, room: &state::Room) {
    let last_id = room
        .message_list
        .messages
        .back()
        .map(|msg| msg.event.event_id.clone());
    let room_id = room.id.clone();
    if let Some(last_read_id) = last_id {
        tokio::task::spawn(async move {
            use matrix_sdk::ruma::api::client::r0::read_marker::set_read_marker::Request;
            let mut request = Request::new(&room_id, &last_read_id);
            request.read_receipt = Some(&last_read_id);
            if let Err(e) = client.send(request, None).await {
                crate::log::error(&format!("{:?}", e));
            }
        });
    }
}

pub fn fetch_old_messages(
    room_id: RoomId,
    room: &mut crate::state::Room,
    client: matrix_sdk::Client,
    tx: tokio::sync::mpsc::UnboundedSender<MatrixEvent>,
) {
    // TODO do this when the SDK also keeps track of the prev_batch token when it comes from
    // discrete request responses
    // let prev_batch = client
    //     .get_joined_room(&room_id)
    //     .map(|r| r.last_prev_batch())
    //     .unwrap_or(None)
    //     .unwrap_or(String::new());
    let prev_batch = match room.prev_batch.clone() {
        Some(pb) => {
            room.prev_batch = None;
            pb
        }
        None => return,
    };
    tokio::task::spawn(async move {
        let mut request =
            matrix_sdk::ruma::api::client::r0::message::get_message_events::Request::backward(
                &room_id,
                &prev_batch,
            );
        request.limit = UInt::new(50).unwrap();

        let room = match client.get_room(&room_id) {
            Some(r) => r,
            None => return,
        };
        let response = match room.messages(request).await {
            Ok(r) => r,
            Err(_) => return,
        };
        if let Some(prev_batch) = response.end {
            if let Err(e) = tx.send(MatrixEvent::PrevBatch {
                id: room_id.clone(),
                prev_batch,
            }) {
                crate::log::error(&e.to_string());
            }
        };
        for event in response.chunk {
            let event = match event.deserialize() {
                Ok(e) => e,
                Err(err) => {
                    crate::log::error(&err.to_string());
                    continue;
                }
            };
            match event {
                AnyRoomEvent::Message(m) => match m {
                    AnyMessageEvent::RoomMessage(evt) => {
                        tx.send(MatrixEvent::OldMessage { event: evt }).unwrap();
                    }
                    AnyMessageEvent::Reaction(evt) => {
                        let relation = evt.content.relates_to;
                        tx.send(MatrixEvent::Reaction {
                            event_id: relation.event_id,
                            user_id: evt.sender,
                            emoji: relation.emoji,
                        })
                        .unwrap();
                    }
                    _ => crate::log::info(&format!("{:?}", m)),
                },
                _ => crate::log::info(&format!("{:?}", event)),
            }
        }
        crate::log::info("state");
        for e in response.state {
            crate::log::info(&format!("{:?}", e));
        }
        crate::log::info("\n");
    });
}

pub struct MatrixBroker {
    pub tx: tokio::sync::mpsc::UnboundedSender<MatrixEvent>,
}

impl MatrixBroker {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<MatrixEvent>) -> Self {
        Self { tx }
    }

    fn publish(&self, event: MatrixEvent) {
        if let Err(err) = self.tx.send(event) {
            crate::log::error(&err.to_string())
        }
    }

    fn handle_timeline(&self, timeline: matrix_sdk::deserialized_responses::Timeline) {
        for event in timeline
            .events
            .iter()
            .filter_map(|e| e.event.deserialize().ok())
        {
            match event {
                AnySyncRoomEvent::Message(msg) => {
                    if let AnySyncMessageEvent::Reaction(evt) = msg {
                        // TODO check if still applicable
                        let relation = evt.content.relates_to;
                        self.publish(MatrixEvent::Reaction {
                            event_id: relation.event_id,
                            user_id: evt.sender,
                            emoji: relation.emoji,
                        });
                    }
                }
                AnySyncRoomEvent::State(_) => {}
                AnySyncRoomEvent::RedactedMessage(_) => {}
                AnySyncRoomEvent::RedactedState(_) => {}
            }
        }
    }

    pub async fn handle_sync_response(
        &self,
        response: matrix_sdk::deserialized_responses::SyncResponse,
    ) -> matrix_sdk::LoopCtrl {
        for (room_id, room) in response.rooms.join {
            self.publish(MatrixEvent::Notifications {
                id: room_id.clone(),
                count: room.unread_notifications.notification_count,
            });
            self.handle_timeline(room.timeline);
        }
        for event in response.to_device.events {
            crate::log::info(&format!("{:?}", event));
        }
        matrix_sdk::LoopCtrl::Continue
    }
}

#[async_trait]
#[allow(unused_must_use)]
impl matrix_sdk::EventHandler for MatrixBroker {
    async fn on_room_member(&self, _: Room, event: &SyncStateEvent<MemberEventContent>) {
        // TODO as ref
        crate::log::info(&format!("on room member {:?}", event));
    }

    async fn on_room_name(&self, room: Room, event: &SyncStateEvent<NameEventContent>) {
        crate::log::info(&format!("on room name {:?}", event));
        if let Room::Joined(room) = room {
            if let Ok(name) = room.display_name().await {
                self.publish(MatrixEvent::RoomName {
                    id: room.room_id().clone(),
                    name,
                });
            }
        }
    }

    async fn on_room_message(&self, room: Room, event: &SyncMessageEvent<MessageEventContent>) {
        if let Room::Joined(room) = room {
            self.publish(MatrixEvent::NewMessage {
                event: event.clone().into_full_event(room.room_id().clone()),
            });
        }
    }

    async fn on_room_message_feedback(
        &self,
        _: Room,
        event: &SyncMessageEvent<FeedbackEventContent>,
    ) {
        crate::log::info(&format!("on room msg fb {:?}", event));
    }

    // TODO never get in here? what is it for?
    async fn on_room_reaction(&self, _: Room, event: &SyncMessageEvent<ReactionEventContent>) {
        self.publish(MatrixEvent::Reaction {
            event_id: event.content.relates_to.event_id.clone(),
            user_id: event.sender.clone(),
            emoji: event.content.relates_to.emoji.clone(),
        });
    }

    async fn on_room_redaction(&self, room_state: Room, event: &SyncRedactionEvent) {
        use matrix_sdk::room::Room::*;
        self.publish(MatrixEvent::Redaction {
            room_id: match room_state {
                Joined(r) => r.room_id().clone(),
                Left(r) => r.room_id().clone(),
                Invited(r) => r.room_id().clone(),
            },
            redacted_id: event.redacts.clone(),
        });
    }

    async fn on_room_power_levels(&self, _: Room, _: &SyncStateEvent<PowerLevelsEventContent>) {}

    async fn on_room_join_rules(&self, _: Room, _: &SyncStateEvent<JoinRulesEventContent>) {}

    async fn on_room_tombstone(&self, _: Room, _: &SyncStateEvent<TombstoneEventContent>) {}

    async fn on_state_member(&self, _: Room, _: &SyncStateEvent<MemberEventContent>) {}

    async fn on_state_name(&self, room: Room, _: &SyncStateEvent<NameEventContent>) {
        // TODO test what happens if I get some history, might start using the older names
        // if so, just keep the time of the latest room name change and use that one
        if let Room::Joined(room) = room {
            if let Ok(name) = room.display_name().await {
                self.publish(MatrixEvent::RoomName {
                    id: room.room_id().clone(),
                    name,
                });
            }
        }
    }

    async fn on_state_canonical_alias(
        &self,
        _: Room,
        _: &SyncStateEvent<CanonicalAliasEventContent>,
    ) {
    }

    async fn on_state_aliases(&self, _: Room, _: &SyncStateEvent<AliasesEventContent>) {}

    async fn on_state_avatar(&self, _: Room, _: &SyncStateEvent<AvatarEventContent>) {}

    async fn on_state_power_levels(&self, _: Room, _: &SyncStateEvent<PowerLevelsEventContent>) {}

    async fn on_state_join_rules(&self, _: Room, _: &SyncStateEvent<JoinRulesEventContent>) {}

    async fn on_stripped_state_member(
        &self,
        _: Room,
        event: &StrippedStateEvent<MemberEventContent>,
        content: Option<MemberEventContent>,
    ) {
        crate::log::info(&format!("on stripped state member {:?}", event));
        crate::log::info(&format!("content {:?}", content));
    }

    async fn on_stripped_state_name(&self, _: Room, _: &StrippedStateEvent<NameEventContent>) {}

    async fn on_stripped_state_canonical_alias(
        &self,
        _: Room,
        _: &StrippedStateEvent<CanonicalAliasEventContent>,
    ) {
    }

    async fn on_stripped_state_aliases(
        &self,
        _: Room,
        _: &StrippedStateEvent<AliasesEventContent>,
    ) {
    }

    async fn on_stripped_state_avatar(&self, _: Room, _: &StrippedStateEvent<AvatarEventContent>) {}

    async fn on_stripped_state_power_levels(
        &self,
        _: Room,
        _: &StrippedStateEvent<PowerLevelsEventContent>,
    ) {
    }

    async fn on_stripped_state_join_rules(
        &self,
        _: Room,
        event: &StrippedStateEvent<JoinRulesEventContent>,
    ) {
        crate::log::info(&format!("on stripped state join rules {:?}", event));
    }

    async fn on_presence_event(&self, _event: &PresenceEvent) {
        // crate::log::info(&format!("on presence event {:?}", event));
    }

    async fn on_unrecognized_event(&self, _: Room, event: &serde_json::value::RawValue) {
        crate::log::info(&format!("on unrecognized {:?}", event));
    }

    async fn on_custom_event(&self, _: Room, event: &matrix_sdk::CustomEvent<'_>) {
        crate::log::info(&format!("on custom event {:?}", event));
    }

    // TODO
    // async fn on_room_call_invite(&self, _: matrix_sdk::room::Room, _: &SyncMessageEvent<call::invite::InviteEventContent>) {}
    // async fn on_room_call_answer(&self, _: matrix_sdk::room::Room, _: &SyncMessageEvent<call::answer::AnswerEventContent>) {}
    // async fn on_room_call_candidates(&self, _: matrix_sdk::room::Room, _: &SyncMessageEvent<call::candidates::CandidatesEventContent>) {}
    // async fn on_room_call_hangup(&self, _: matrix_sdk::room::Room, _: &SyncMessageEvent<call::hangup::HangupEventContent>) {}

    // TODO?
    // async fn on_room_notification(&self, _: matrix_sdk::room::Room, _: matrix_sdk::api::r0::push::get_notifications::Notification) {}
}
