use async_trait::async_trait;

use matrix_sdk::{events::*, identifiers::RoomId};

use crate::{events::*, state::Room};

pub fn send_read_receipt_current_room(client: matrix_sdk::Client, room: &Room) {
    let last_id = room
        .message_list
        .messages
        .back()
        .map(|msg| msg.event.event_id.clone());
    let room_id = room.id.clone();
    if let Some(last_read_id) = last_id {
        tokio::task::spawn(async move {
            use matrix_sdk::api::r0::read_marker::set_read_marker::Request;
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
        let mut request = matrix_sdk::api::r0::message::get_message_events::Request::backward(
            &room_id,
            &prev_batch,
        );
        request.limit = matrix_sdk::UInt::new(50).unwrap();

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
            crate::log::info(format!("{:?}", event).as_ref());
        }
        matrix_sdk::LoopCtrl::Continue
    }
}

#[async_trait]
#[allow(unused_must_use)]
impl matrix_sdk::EventHandler for MatrixBroker {
    async fn on_room_member(
        &self,
        _: matrix_sdk::room::Room,
        event: &SyncStateEvent<room::member::MemberEventContent>,
    ) {
        // TODO as ref
        crate::log::info(format!("on room member {:?}", event).as_ref());
    }

    async fn on_room_name(
        &self,
        room: matrix_sdk::room::Room,
        event: &SyncStateEvent<room::name::NameEventContent>,
    ) {
        crate::log::info(format!("on room name {:?}", event).as_ref());
        if let matrix_sdk::room::Room::Joined(room) = room {
            if let Ok(name) = room.display_name().await {
                self.publish(MatrixEvent::RoomName {
                    id: room.room_id().clone(),
                    name,
                });
            }
        }
    }

    async fn on_room_message(
        &self,
        room: matrix_sdk::room::Room,
        event: &SyncMessageEvent<room::message::MessageEventContent>,
    ) {
        if let matrix_sdk::room::Room::Joined(room) = room {
            self.publish(MatrixEvent::NewMessage {
                event: event.clone().into_full_event(room.room_id().clone()),
            });
        }
    }

    async fn on_room_message_feedback(
        &self,
        _: matrix_sdk::room::Room,
        event: &SyncMessageEvent<room::message::feedback::FeedbackEventContent>,
    ) {
        crate::log::info(format!("on room msg fb {:?}", event).as_ref());
    }

    async fn on_room_redaction(
        &self,
        room_state: matrix_sdk::room::Room,
        event: &room::redaction::SyncRedactionEvent,
    ) {
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

    async fn on_room_power_levels(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::power_levels::PowerLevelsEventContent>,
    ) {
    }

    async fn on_room_join_rules(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::join_rules::JoinRulesEventContent>,
    ) {
    }

    async fn on_room_tombstone(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::tombstone::TombstoneEventContent>,
    ) {
    }

    async fn on_state_member(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::member::MemberEventContent>,
    ) {
    }

    async fn on_state_name(
        &self,
        room: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::name::NameEventContent>,
    ) {
        // TODO test what happens if I get some history, might start using the older names
        // if so, just keep the time of the latest room name change and use that one
        if let matrix_sdk::room::Room::Joined(room) = room {
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
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::canonical_alias::CanonicalAliasEventContent>,
    ) {
    }

    async fn on_state_aliases(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::aliases::AliasesEventContent>,
    ) {
    }

    async fn on_state_avatar(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::avatar::AvatarEventContent>,
    ) {
    }

    async fn on_state_power_levels(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::power_levels::PowerLevelsEventContent>,
    ) {
    }

    async fn on_state_join_rules(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncStateEvent<room::join_rules::JoinRulesEventContent>,
    ) {
    }

    async fn on_stripped_state_member(
        &self,
        _: matrix_sdk::room::Room,
        event: &StrippedStateEvent<room::member::MemberEventContent>,
        content: Option<room::member::MemberEventContent>,
    ) {
        crate::log::info(format!("on stripped state member {:?}", event).as_ref());
        crate::log::info(format!("content {:?}", content).as_ref());
    }

    async fn on_stripped_state_name(
        &self,
        _: matrix_sdk::room::Room,
        _: &StrippedStateEvent<room::name::NameEventContent>,
    ) {
    }

    async fn on_stripped_state_canonical_alias(
        &self,
        _: matrix_sdk::room::Room,
        _: &StrippedStateEvent<room::canonical_alias::CanonicalAliasEventContent>,
    ) {
    }

    async fn on_stripped_state_aliases(
        &self,
        _: matrix_sdk::room::Room,
        _: &StrippedStateEvent<room::aliases::AliasesEventContent>,
    ) {
    }

    async fn on_stripped_state_avatar(
        &self,
        _: matrix_sdk::room::Room,
        _: &StrippedStateEvent<room::avatar::AvatarEventContent>,
    ) {
    }

    async fn on_stripped_state_power_levels(
        &self,
        _: matrix_sdk::room::Room,
        _: &StrippedStateEvent<room::power_levels::PowerLevelsEventContent>,
    ) {
    }

    async fn on_stripped_state_join_rules(
        &self,
        _: matrix_sdk::room::Room,
        event: &StrippedStateEvent<room::join_rules::JoinRulesEventContent>,
    ) {
        crate::log::info(format!("on stripped state join rules {:?}", event).as_ref());
    }

    async fn on_non_room_presence(
        &self,
        _: matrix_sdk::room::Room,
        event: &presence::PresenceEvent,
    ) {
        crate::log::info(format!("on non room presence event {:?}", event).as_ref());
    }

    async fn on_non_room_typing(
        &self,
        _: matrix_sdk::room::Room,
        _: &SyncEphemeralRoomEvent<typing::TypingEventContent>,
    ) {
    }

    async fn on_non_room_receipt(
        &self,
        _: matrix_sdk::room::Room,
        _event: &SyncEphemeralRoomEvent<receipt::ReceiptEventContent>,
    ) {
    }

    async fn on_presence_event(&self, _event: &presence::PresenceEvent) {
        // crate::log::info(format!("on presence event {:?}", event).as_ref());
    }

    async fn on_unrecognized_event(
        &self,
        _: matrix_sdk::room::Room,
        event: &exports::serde_json::value::RawValue,
    ) {
        crate::log::info(format!("on unrecognized {:?}", event).as_ref());
    }

    async fn on_custom_event(
        &self,
        _: matrix_sdk::room::Room,
        event: &matrix_sdk::CustomEvent<'_>,
    ) {
        crate::log::info(format!("on custom event {:?}", event).as_ref());
    }
}
