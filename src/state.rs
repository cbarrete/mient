use std::collections::{HashMap, VecDeque};
use std::time::SystemTime;

use matrix_sdk::events::room::message::MessageEventContent;
use matrix_sdk::identifiers::RoomId;
use matrix_sdk::identifiers::UserId;

use crate::events::Event;

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
    pub current_index: usize,
}

impl MessageList {
    fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            current_index: 0,
        }
    }

    pub fn push_new(&mut self, message: Message) {
        if self.current_index == self.messages.len() {
            self.current_index += 1;
        }
        self.messages.push_back(message);
    }

    pub fn push_old(&mut self, message: Message) {
        self.messages.push_front(message);
        self.current_index += 1;
    }
}

#[derive(Debug)]
pub struct Room {
    pub name: String,
    pub id: RoomId,
    pub message_list: MessageList,
    // TODO maybe just always get it from the SDK
    pub notifications: u64,
}

impl Room {
    pub fn new(name: String, id: RoomId, notifications: u64) -> Self {
        Self {
            name,
            id,
            message_list: MessageList::new(),
            notifications,
        }
    }
}

pub struct State {
    pub input: String,
    pub current_room_index: usize,
    pub users: HashMap<UserId, String>,
    pub rooms: Vec<Room>,
}

impl State {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            current_room_index: 0,
            users: HashMap::new(),
            rooms: Vec::new(),
        }
    }

    pub fn get_current_room(&self) -> Option<&Room> {
        self.rooms.get(self.current_room_index)
    }

    #[allow(dead_code)]
    pub fn get_current_room_mut(&mut self) -> Option<&mut Room> {
        self.rooms.get_mut(self.current_room_index)
    }

    #[allow(dead_code)]
    pub fn get_room(&self, room_id: &RoomId) -> Option<&Room> {
        for room in &self.rooms {
            if &room.id == room_id {
                return Some(&room);
            }
        }
        None
    }

    pub fn get_room_mut(&mut self, room_id: &RoomId) -> Option<&mut Room> {
        for room in self.rooms.iter_mut() {
            if &room.id == room_id {
                return Some(room);
            }
        }
        None
    }

    pub async fn populate(
        &mut self,
        client: matrix_sdk::Client,
        tx: tokio::sync::mpsc::UnboundedSender<Event>,
    ) {
        let joined_rooms = client.joined_rooms();
        for room in joined_rooms {
            let mient_room = Room::new(
                room.display_name().await.unwrap(),
                room.room_id().clone(),
                room.unread_notification_counts().notification_count,
            );
            self.rooms.push(mient_room);

            // TODO get initial state from state store when the SDK supports it
            crate::matrix::fetch_old_messages(room.room_id().clone(), client.clone(), tx.clone())
        }
    }

    pub fn change_current_room(&mut self, increment: i8) {
        self.current_room_index =
            (self.current_room_index as i8 + increment).rem_euclid(self.rooms.len() as i8) as usize;
    }

    pub fn change_current_message(&mut self, increment: i8) {
        if let Some(current_room) = self.get_current_room_mut() {
            let message_list = &mut current_room.message_list;
            message_list.current_index = (message_list.current_index as i8 + increment)
                .clamp(0, message_list.messages.len() as i8)
                as usize;
        }
    }
}
