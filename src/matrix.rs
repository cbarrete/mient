use async_trait::async_trait;

use matrix_sdk::events::*;

use crate::events::*;
use crate::state::Message;

pub struct MatrixBroker {
    pub tx: tokio::sync::mpsc::UnboundedSender<Event>,
}

impl MatrixBroker {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<Event>) -> Self {
        Self { tx }
    }

    fn publish(&self, event: MatrixEvent) {
        if let Err(err) = self.tx.send(Event::Matrix(event)) {
            crate::log::error(&err.to_string())
        }
    }
}

impl MatrixBroker {
    pub async fn handle_sync_response(
        &self,
        response: matrix_sdk::deserialized_responses::SyncResponse,
    ) -> matrix_sdk::LoopCtrl {
        for (room_id, room) in response.rooms.join {
            if let Some(prev_batch) = room.timeline.prev_batch {
                self.publish(MatrixEvent::PrevBatch {
                    id: room_id.clone(),
                    prev_batch,
                })
            }
            self.publish(MatrixEvent::Notifications {
                id: room_id.clone(),
                count: room.unread_notifications.notification_count,
            });
        }
        for event in response.to_device.events {
            self.handle_to_device(event);
        }
        matrix_sdk::LoopCtrl::Continue
    }

    fn handle_to_device(&self, event: AnyToDeviceEvent) {
        crate::log::info(format!("{:?}", event).as_ref());
    }
}

#[async_trait]
#[allow(unused_must_use)]
impl matrix_sdk::EventEmitter for MatrixBroker {
    async fn on_room_member(
        &self,
        _: matrix_sdk::RoomState,
        event: &SyncStateEvent<room::member::MemberEventContent>,
    ) {
        crate::log::info(format!("on room member {:?}", event).as_ref());
    }

    async fn on_room_name(
        &self,
        room: matrix_sdk::RoomState,
        event: &SyncStateEvent<room::name::NameEventContent>,
    ) {
        crate::log::info(format!("on room name {:?}", event).as_ref());
        if let matrix_sdk::RoomState::Joined(room) = room {
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
        room: matrix_sdk::RoomState,
        event: &SyncMessageEvent<room::message::MessageEventContent>,
    ) {
        if let matrix_sdk::RoomState::Joined(room) = room {
            self.publish(MatrixEvent::NewMessage {
                id: room.room_id().clone(),
                message: Message::new(
                    event.sender.clone(),
                    event.content.clone(),
                    event.origin_server_ts.clone(),
                ),
            });
        }
    }

    async fn on_room_message_feedback(
        &self,
        _: matrix_sdk::RoomState,
        event: &SyncMessageEvent<room::message::feedback::FeedbackEventContent>,
    ) {
        crate::log::info(format!("on room msg fb {:?}", event).as_ref());
    }

    async fn on_room_redaction(
        &self,
        _: matrix_sdk::RoomState,
        event: &room::redaction::SyncRedactionEvent,
    ) {
        crate::log::info(format!("on room redaction {:?}", event).as_ref());
    }

    async fn on_room_power_levels(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::power_levels::PowerLevelsEventContent>,
    ) {
    }

    async fn on_room_join_rules(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::join_rules::JoinRulesEventContent>,
    ) {
    }

    async fn on_room_tombstone(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::tombstone::TombstoneEventContent>,
    ) {
    }

    async fn on_state_member(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::member::MemberEventContent>,
    ) {
    }

    async fn on_state_name(
        &self,
        room: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::name::NameEventContent>,
    ) {
        // TODO test what happens if I get some history, might start using the older names
        // if so, just keep the time of the latest room name change and use that one
        if let matrix_sdk::RoomState::Joined(room) = room {
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
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::canonical_alias::CanonicalAliasEventContent>,
    ) {
    }

    async fn on_state_aliases(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::aliases::AliasesEventContent>,
    ) {
    }

    async fn on_state_avatar(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::avatar::AvatarEventContent>,
    ) {
    }

    async fn on_state_power_levels(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::power_levels::PowerLevelsEventContent>,
    ) {
    }

    async fn on_state_join_rules(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncStateEvent<room::join_rules::JoinRulesEventContent>,
    ) {
    }

    async fn on_stripped_state_member(
        &self,
        _: matrix_sdk::RoomState,
        event: &StrippedStateEvent<room::member::MemberEventContent>,
        content: Option<room::member::MemberEventContent>,
    ) {
        crate::log::info(format!("on stripped state member {:?}", event).as_ref());
        crate::log::info(format!("content {:?}", content).as_ref());
    }

    async fn on_stripped_state_name(
        &self,
        _: matrix_sdk::RoomState,
        _: &StrippedStateEvent<room::name::NameEventContent>,
    ) {
    }

    async fn on_stripped_state_canonical_alias(
        &self,
        _: matrix_sdk::RoomState,
        _: &StrippedStateEvent<room::canonical_alias::CanonicalAliasEventContent>,
    ) {
    }

    async fn on_stripped_state_aliases(
        &self,
        _: matrix_sdk::RoomState,
        _: &StrippedStateEvent<room::aliases::AliasesEventContent>,
    ) {
    }

    async fn on_stripped_state_avatar(
        &self,
        _: matrix_sdk::RoomState,
        _: &StrippedStateEvent<room::avatar::AvatarEventContent>,
    ) {
    }

    async fn on_stripped_state_power_levels(
        &self,
        _: matrix_sdk::RoomState,
        _: &StrippedStateEvent<room::power_levels::PowerLevelsEventContent>,
    ) {
    }

    async fn on_stripped_state_join_rules(
        &self,
        _: matrix_sdk::RoomState,
        event: &StrippedStateEvent<room::join_rules::JoinRulesEventContent>,
    ) {
        crate::log::info(format!("on stripped state join rules {:?}", event).as_ref());
    }

    async fn on_non_room_presence(
        &self,
        _: matrix_sdk::RoomState,
        event: &presence::PresenceEvent,
    ) {
        crate::log::info(format!("on non room presence event {:?}", event).as_ref());
    }

    async fn on_non_room_ignored_users(
        &self,
        _: matrix_sdk::RoomState,
        _: &BasicEvent<ignored_user_list::IgnoredUserListEventContent>,
    ) {
    }

    async fn on_non_room_push_rules(
        &self,
        _: matrix_sdk::RoomState,
        _: &BasicEvent<push_rules::PushRulesEventContent>,
    ) {
    }

    async fn on_non_room_fully_read(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncEphemeralRoomEvent<fully_read::FullyReadEventContent>,
    ) {
    }

    async fn on_non_room_typing(
        &self,
        _: matrix_sdk::RoomState,
        _: &SyncEphemeralRoomEvent<typing::TypingEventContent>,
    ) {
    }

    async fn on_non_room_receipt(
        &self,
        _: matrix_sdk::RoomState,
        _event: &SyncEphemeralRoomEvent<receipt::ReceiptEventContent>,
    ) {
    }

    async fn on_presence_event(&self, _event: &presence::PresenceEvent) {
        // crate::log::info(format!("on presence event {:?}", event).as_ref());
    }

    async fn on_unrecognized_event(
        &self,
        _: matrix_sdk::RoomState,
        event: &exports::serde_json::value::RawValue,
    ) {
        crate::log::info(format!("on unrecognized {:?}", event).as_ref());
    }

    async fn on_custom_event(&self, _: matrix_sdk::RoomState, event: &matrix_sdk::CustomEvent<'_>) {
        crate::log::info(format!("on custom event {:?}", event).as_ref());
    }
}
