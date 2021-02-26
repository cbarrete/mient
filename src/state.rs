use std::collections::{HashMap, HashSet, VecDeque};
use std::time::SystemTime;

use matrix_sdk::identifiers::RoomId;
use matrix_sdk::identifiers::UserId;
use matrix_sdk::{events::room::message::MessageEventContent, identifiers::EventId};

use crate::events::MatrixEvent;

#[derive(Debug, Clone)]
pub struct Message {
    pub sender: UserId,
    pub id: EventId,
    pub content: MessageEventContent,
    pub ts: SystemTime,
}

impl Message {
    pub fn new(sender: UserId, id: EventId, content: MessageEventContent, ts: SystemTime) -> Self {
        Self {
            sender,
            id,
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
    pub prev_batch: Option<String>,
}

impl Room {
    pub fn new(name: String, id: RoomId, notifications: u64, prev_batch: Option<String>) -> Self {
        Self {
            name,
            id,
            message_list: MessageList::new(),
            notifications,
            prev_batch,
        }
    }
}

pub struct State {
    pub user_id: UserId,
    pub input: String,
    pub current_room_index: usize,
    pub users: HashMap<UserId, String>,
    pub rooms: Vec<Room>,
    pub reactions: HashMap<EventId, HashMap<String, HashSet<UserId>>>,
}

impl State {
    pub async fn from_client(
        client: matrix_sdk::Client,
        tx: tokio::sync::mpsc::UnboundedSender<MatrixEvent>,
    ) -> Self {
        let mut rooms = Vec::new();
        for room in client.joined_rooms() {
            // TODO get initial state from state store when the SDK supports it
            let prev_batch = client
                .get_joined_room(room.room_id())
                .map(|r| r.last_prev_batch())
                .unwrap_or(None);
            let mut mient_room = Room::new(
                room.display_name().await.unwrap(),
                room.room_id().clone(),
                room.unread_notification_counts().notification_count,
                prev_batch,
            );

            crate::matrix::fetch_old_messages(
                room.room_id().clone(),
                &mut mient_room,
                client.clone(),
                tx.clone(),
            );
            rooms.push(mient_room);
        }
        Self {
            input: String::new(),
            current_room_index: 0,
            users: HashMap::new(),
            rooms,
            user_id: client.user_id().await.unwrap(),
            reactions: HashMap::new(),
        }
    }

    pub fn current_room(&self) -> Option<&Room> {
        self.rooms.get(self.current_room_index)
    }

    #[allow(dead_code)]
    pub fn current_room_mut(&mut self) -> Option<&mut Room> {
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

    pub fn change_current_room(&mut self, increment: i32) {
        self.current_room_index = (self.current_room_index as i32 + increment)
            .rem_euclid(self.rooms.len() as i32) as usize;
    }

    pub fn change_current_message(&mut self, increment: i32) {
        if let Some(current_room) = self.current_room_mut() {
            let message_list = &mut current_room.message_list;
            message_list.current_index = (message_list.current_index as i32 + increment)
                .clamp(0, message_list.messages.len() as i32)
                as usize;
        }
    }
}
