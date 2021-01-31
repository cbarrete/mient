use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;

use matrix_sdk::events::room::message::MessageEventContent;
use matrix_sdk::identifiers::RoomId;
use matrix_sdk::identifiers::UserId;

#[derive(Debug)]
pub struct Message {
    pub sender: UserId,
    pub content: MessageEventContent,
    pub ts: SystemTime,
}

impl Message {
    pub fn new(sender: UserId, content: MessageEventContent, ts: SystemTime) -> Self {
        Self {
            sender,
            content,
            ts,
        }
    }
}

#[derive(Debug)]
pub struct MessageList {
    pub messages: VecDeque<Message>,
}

impl MessageList {
    fn new() -> Self {
        Self {
            messages: VecDeque::new(),
        }
    }

    pub fn push_new(&mut self, message: Message) {
        self.messages.push_back(message)
    }

    pub fn push_old(&mut self, message: Message) {
        self.messages.push_front(message)
    }
}

#[derive(Debug)]
pub struct Room {
    pub name: String,
    pub message_list: MessageList,
    pub notifications: matrix_sdk::UInt,
    pub prev_batch: String,
}

impl Room {
    pub fn new(name: String, notifications: matrix_sdk::UInt) -> Self {
        Self {
            name,
            message_list: MessageList::new(),
            notifications,
            prev_batch: String::new(),
        }
    }
}

pub struct State {
    pub input: String,
    pub current_room_id: Option<RoomId>,
    pub users: HashMap<UserId, String>,
    pub rooms: Vec<(RoomId, Box<Room>)>,
}

impl State {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            current_room_id: None,
            users: HashMap::new(),
            rooms: Vec::new(),
        }
    }

    pub fn get_current_room(&self) -> Option<&Room> {
        if let Some(id) = &self.current_room_id {
            self.get_room(&id)
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn get_current_room_mut(&mut self) -> Option<&mut Room> {
        if let Some(id) = self.current_room_id.clone() {
            self.get_room_mut(&id)
        } else {
            None
        }
    }

    pub fn get_room(&self, room_id: &RoomId) -> Option<&Room> {
        for (id, room) in &self.rooms {
            if id == room_id {
                return Some(&room);
            }
        }
        None
    }

    pub fn get_room_mut(&mut self, room_id: &RoomId) -> Option<&mut Room> {
        for (id, room) in self.rooms.iter_mut() {
            if id == room_id {
                return Some(room);
            }
        }
        None
    }

    pub async fn populate(&mut self, client: &matrix_sdk::Client) {
        let joined_rooms = client.joined_rooms();
        let joined_rooms = joined_rooms.read().await;
        for (room_id, room) in joined_rooms.iter() {
            let room_ref = room.read().await;
            let mut room = Room::new(
                room_ref.display_name(),
                room_ref.unread_notifications.unwrap_or_default(),
            );
            for event in room_ref.messages.iter() {
                if let matrix_sdk::events::AnyPossiblyRedactedSyncMessageEvent::Regular(msg) = event
                {
                    // dropping non text messages for now
                    if let matrix_sdk::events::AnySyncMessageEvent::RoomMessage(msg_event) = msg {
                        room.message_list.push_new(Message::new(
                            msg.sender().clone(),
                            msg_event.content.clone(),
                            msg.origin_server_ts().clone(),
                        ));
                    }
                }
            }
            self.rooms.push((room_id.clone(), Box::new(room)));
        }
    }

    pub fn change_current_room(&mut self, increment: i8) {
        let current_position = if let Some(current_id) = &self.current_room_id {
            self.rooms.iter().position(|(id, _)| &*id == current_id)
        } else {
            None
        };
        let new_position = current_position
            .map(|p| (p as i8 + increment).rem_euclid(self.rooms.len() as i8))
            .unwrap_or(0);
        self.current_room_id = self
            .rooms
            .get(new_position as usize)
            .map(|(id, _)| id.clone());
    }
}
