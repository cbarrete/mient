use async_trait::async_trait;

use matrix_sdk::events::*;

use crate::events::*;
use crate::state::Message;
use crate::log::log;

pub struct MatrixBroker {
    pub tx: tokio::sync::mpsc::UnboundedSender<Event>,
}

impl MatrixBroker {
    pub fn new(tx: tokio::sync::mpsc::UnboundedSender<Event>) -> Self {
        Self { tx }
    }

    fn publish(&self, event: MatrixEvent) {
        self.tx.send(Event::Matrix(event));
        // TODO log if couldn't send
    }

    fn debug(&self, message: &str) {
        log(message)
        // self.tx.send(Event::Debug(String::from(message))).unwrap();
    }
}

impl MatrixBroker {
    pub async fn handle_response(&self, response: matrix_sdk::api::r0::sync::sync_events::Response) -> matrix_sdk::LoopCtrl {
        for (room_id, room) in response.rooms.join {
            if let Some(count) = room.unread_notifications.notification_count {
                self.publish(MatrixEvent::Notifications { id: room_id.clone(), count });
            }
        }
        for event in response.to_device.events.iter().filter_map(|e| e.deserialize().ok()) {
            self.handle_to_device(event);
        }
        matrix_sdk::LoopCtrl::Continue
    }

    fn handle_to_device(&self, event: AnyToDeviceEvent) {
        self.debug(format!("{:?}", event).as_ref());
    }
}

#[async_trait]
#[allow(unused_must_use)]
impl matrix_sdk::EventEmitter for MatrixBroker {
    async fn on_room_member(&self, _: matrix_sdk::SyncRoom, event: &SyncStateEvent<room::member::MemberEventContent>) {
        self.debug(format!("on room member {:?}", event).as_ref());
    }

    async fn on_room_name(&self, room: matrix_sdk::SyncRoom, event: &SyncStateEvent<room::name::NameEventContent>) {
        self.debug(format!("on room name {:?}", event).as_ref());
        if let matrix_sdk::RoomState::Joined(room) = room {
            let room = room.read().await;
            let name = room.display_name();
            self.publish(MatrixEvent::RoomName { id: room.room_id.clone(), name });
        }
    }

    async fn on_room_message(&self, room: matrix_sdk::SyncRoom, event: &SyncMessageEvent<room::message::MessageEventContent>) {
        if let matrix_sdk::SyncRoom::Joined(room) = room {
            let room = room.read().await;
            self.publish(MatrixEvent::Message {
                id: room.room_id.clone(),
                message: Message::new(event.sender.clone(), event.content.clone(), event.origin_server_ts.clone())
            });
        }
    }

    async fn on_room_message_feedback(
        &self,
        _: matrix_sdk::SyncRoom,
        event: &SyncMessageEvent<room::message::feedback::FeedbackEventContent>,
    ) {
        self.debug(format!("on room msg fb {:?}", event).as_ref());
    }

    async fn on_room_redaction(&self, _: matrix_sdk::SyncRoom, event: &room::redaction::SyncRedactionEvent) {
        self.debug(format!("on room redaction {:?}", event).as_ref());
    }

    async fn on_room_power_levels(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::power_levels::PowerLevelsEventContent>) {
    }

    async fn on_room_join_rules(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::join_rules::JoinRulesEventContent>) {}

    async fn on_room_tombstone(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::tombstone::TombstoneEventContent>) {}

    async fn on_state_member(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::member::MemberEventContent>) {}

    async fn on_state_name(&self, room: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::name::NameEventContent>) {
        // TODO test what happens if I get some history, might start using the older names
        // if so, just keep the time of the latest room name change and use that one
        if let matrix_sdk::RoomState::Joined(room) = room {
            let room = room.read().await;
            self.publish(MatrixEvent::RoomName { id: room.room_id.clone(), name: room.display_name() });
        }
    }

    async fn on_state_canonical_alias(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::canonical_alias::CanonicalAliasEventContent>) {
    }

    async fn on_state_aliases(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::aliases::AliasesEventContent>) {}

    async fn on_state_avatar(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::avatar::AvatarEventContent>) {}

    async fn on_state_power_levels(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::power_levels::PowerLevelsEventContent>) {
    }

    async fn on_state_join_rules(&self, _: matrix_sdk::SyncRoom, _: &SyncStateEvent<room::join_rules::JoinRulesEventContent>) {}

    async fn on_stripped_state_member(&self, _: matrix_sdk::SyncRoom, event: &StrippedStateEvent<room::member::MemberEventContent>, content: Option<room::member::MemberEventContent>) {
        self.debug(format!("on stripped state member {:?}", event).as_ref());
        self.debug(format!("content {:?}", content).as_ref());
    }

    async fn on_stripped_state_name(&self, _: matrix_sdk::SyncRoom, _: &StrippedStateEvent<room::name::NameEventContent>) {}

    async fn on_stripped_state_canonical_alias( &self, _: matrix_sdk::SyncRoom, _: &StrippedStateEvent<room::canonical_alias::CanonicalAliasEventContent>) {
    }

    async fn on_stripped_state_aliases(&self, _: matrix_sdk::SyncRoom, _: &StrippedStateEvent<room::aliases::AliasesEventContent>) {
    }

    async fn on_stripped_state_avatar(&self, _: matrix_sdk::SyncRoom, _: &StrippedStateEvent<room::avatar::AvatarEventContent>) {
    }

    async fn on_stripped_state_power_levels(&self, _: matrix_sdk::SyncRoom, _: &StrippedStateEvent<room::power_levels::PowerLevelsEventContent>) {
    }

    async fn on_stripped_state_join_rules(&self, _: matrix_sdk::SyncRoom, event: &StrippedStateEvent<room::join_rules::JoinRulesEventContent>) {
        self.debug(format!("on stripped state join rules {:?}", event).as_ref());
    }

    async fn on_non_room_presence(&self, _: matrix_sdk::SyncRoom, event: &presence::PresenceEvent) {
        self.debug(format!("on non room presence event {:?}", event).as_ref());
    }

    async fn on_non_room_ignored_users( &self, _: matrix_sdk::SyncRoom, _: &BasicEvent<ignored_user_list::IgnoredUserListEventContent>) {
    }

    async fn on_non_room_push_rules(&self, _: matrix_sdk::SyncRoom, _: &BasicEvent<push_rules::PushRulesEventContent>) {}

    async fn on_non_room_fully_read(&self, _: matrix_sdk::SyncRoom, _: &SyncEphemeralRoomEvent<fully_read::FullyReadEventContent>) {
    }

    async fn on_non_room_typing(&self, _: matrix_sdk::SyncRoom, _: &SyncEphemeralRoomEvent<typing::TypingEventContent>) {}

    async fn on_non_room_receipt(&self, _: matrix_sdk::SyncRoom, _event: &SyncEphemeralRoomEvent<receipt::ReceiptEventContent>) {}

    async fn on_presence_event(&self, _: matrix_sdk::SyncRoom, _event: &presence::PresenceEvent) {
        // self.debug(format!("on presence event {:?}", event).as_ref());
    }

    async fn on_unrecognized_event(&self, _: matrix_sdk::SyncRoom, event: &exports::serde_json::value::RawValue) {
        self.debug(format!("on unrecognized {:?}", event).as_ref());
    }

    async fn on_custom_event(&self, _: matrix_sdk::SyncRoom, event: &matrix_sdk::CustomEvent<'_>) {
        self.debug(format!("on custom event {:?}", event).as_ref());
    }
}

