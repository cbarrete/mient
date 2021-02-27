use std::collections::{HashMap, HashSet};

use matrix_sdk::{events::MessageEvent, identifiers::{EventId, RoomId, UserId}};
use matrix_sdk::events::room::message::MessageEventContent;
use matrix_sdk::uuid::Uuid;
use termion::event::Key;

use crate::state::Room;
use crate::state::State;

#[derive(Debug)]
pub struct UserEvent;

#[derive(Debug)]
pub enum MatrixEvent {
    RoomName {
        id: RoomId,
        name: String,
    },
    NewMessage {
        message: MessageEvent<MessageEventContent>,
    },
    OldMessage {
        message: MessageEvent<MessageEventContent>,
    },
    Notifications {
        id: RoomId,
        count: u64,
    },
    PrevBatch {
        id: RoomId,
        prev_batch: String,
    },
    Reaction {
        event_id: EventId,
        user_id: UserId,
        emoji: String,
    },
}

#[derive(Debug)]
pub enum MientEvent {
    Keyboard(Key),
    Tick,
}

fn handle_keyboard_event(
    key: Key,
    state: &mut State,
    client: &mut matrix_sdk::Client,
    tx: &tokio::sync::mpsc::UnboundedSender<MatrixEvent>,
) -> bool {
    match key {
        Key::Char('\n') => {
            if let Some(room) = state.current_room() {
                if state.input.is_empty() {
                    return true;
                }
                use matrix_sdk::events::room::message;
                let id = room.id.clone();
                let selected_message = room
                    .message_list
                    .messages
                    .get(room.message_list.current_index)
                    .map(|m| m.clone());

                let text: String = state.input.drain(..).collect();
                let mut text_content = message::TextMessageEventContent::plain(text.clone());

                if let Some(msg) = selected_message.clone() {
                    use matrix_sdk::events::room::relationships;
                    let relates_to = message::Relation::Reply {
                        in_reply_to: relationships::InReplyTo {
                            event_id: msg.event_id.clone(),
                        },
                    };
                    text_content.relates_to = Some(relates_to);
                    text_content.body =
                        crate::utils::format_reply_content(msg.content, msg.sender, text);
                };

                let content = message::MessageEventContent::Text(text_content);
                let message = matrix_sdk::events::AnyMessageEventContent::RoomMessage(content);
                let client = client.clone();
                tokio::task::spawn(async move { client.room_send(&id, message, None).await });
                // TODO txn id for local echo
            }
        }
        Key::Char(c) => {
            state.input.push(c);
        }
        Key::Backspace => {
            state.input.pop();
        }
        Key::Ctrl('u') => state.input.clear(),
        Key::Ctrl('p') => state.change_current_room(-1),
        Key::Ctrl('n') => state.change_current_room(1),
        Key::Up | Key::Home => {
            if let Some(mut room) = state.current_room_mut() {
                if room.message_list.current_index == 0 {
                    crate::matrix::fetch_old_messages(
                        room.id.clone(),
                        &mut room,
                        client.clone(),
                        tx.clone(),
                    );
                } else {
                    let position = match key {
                        Key::Up => crate::state::ListPosition::Relative(-1),
                        Key::Home => crate::state::ListPosition::First,
                        _ => unreachable!(),
                    };
                    state.change_current_message(position);
                }
            }
        }
        Key::Down => state.change_current_message(crate::state::ListPosition::Relative(1)),
        Key::End => state.change_current_message(crate::state::ListPosition::Last),
        Key::Delete => {
            let room = match state.current_room() {
                Some(r) => {r},
                None => {return true},
            };
            let selected_message = room
                .message_list
                .messages
                .get(room.message_list.current_index)
                .map(|m| m.clone());
            if let Some(msg) = selected_message {
                use matrix_sdk::api::r0::redact::redact_event::Request;
                let txn_id = Uuid::new_v4().to_string();
                let client = client.clone();
                tokio::task::spawn(async move {
                    let request = Request::new(&msg.room_id, &msg.event_id, &txn_id);
                    if let Err(e) = client.send(request, None).await {
                        crate::log::error(&format!("{:?}", e));
                    }
                });
            }
        }
        Key::Esc => return false,
        _ => {}
    };
    true
}

pub async fn handle_mient_event(
    event: MientEvent,
    state: &mut State,
    client: &mut matrix_sdk::Client,
    tx: &tokio::sync::mpsc::UnboundedSender<MatrixEvent>,
) -> bool {
    match event {
        MientEvent::Keyboard(key) => handle_keyboard_event(key, state, client, &tx),
        MientEvent::Tick => true,
    }
}

pub async fn handle_matrix_event(event: MatrixEvent, state: &mut State) {
    match event {
        MatrixEvent::RoomName { id, name } => match state.get_room_mut(&id) {
            Some(room) => room.name = name,
            None => state.rooms.push(Room::new(name, id, 0, None)),
        },
        MatrixEvent::NewMessage { message } => {
            if let Some(room) = state.get_room_mut(&message.room_id) {
                room.message_list.push_new(message)
            }
        }
        MatrixEvent::OldMessage { message } => {
            if let Some(room) = state.get_room_mut(&message.room_id) {
                room.message_list.push_old(message)
            }
        }
        MatrixEvent::Notifications { id, count } => {
            state
                .get_room_mut(&id)
                .map(|room| room.notifications = count);
        }
        MatrixEvent::PrevBatch { id, prev_batch } => {
            state
                .get_room_mut(&id)
                .map(|room| room.prev_batch = Some(prev_batch));
        }
        MatrixEvent::Reaction {
            event_id,
            user_id,
            emoji,
        } => {
            state
                .reactions
                .entry(event_id)
                .or_insert_with(|| HashMap::new())
                .entry(emoji)
                .or_insert_with(|| HashSet::new())
                .insert(user_id);
        }
    }
}
