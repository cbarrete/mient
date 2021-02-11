use matrix_sdk::identifiers::RoomId;
use termion::event::Key;

use crate::state::Message;
use crate::state::Room;
use crate::state::State;

#[derive(Debug)]
pub struct UserEvent;

#[derive(Debug)]
pub enum MatrixEvent {
    RoomName { id: RoomId, name: String },
    NewMessage { id: RoomId, message: Message },
    OldMessage { id: RoomId, message: Message },
    Notifications { id: RoomId, count: u64 },
    PrevBatch { id: RoomId, prev_batch: String },
}

#[derive(Debug)]
pub enum Event {
    Keyboard(Key),
    Matrix(MatrixEvent),
    Tick,
}

fn handle_keyboard_event(
    key: Key,
    state: &mut State,
    client: &mut matrix_sdk::Client,
    tx: tokio::sync::mpsc::UnboundedSender<Event>,
) -> bool {
    match key {
        Key::Char('\n') => {
            if let Some(id) = &state.current_room_id {
                if state.input.is_empty() {
                    return true;
                }
                let text: String = state.input.drain(..).collect();
                let content =
                    matrix_sdk::events::room::message::MessageEventContent::text_plain(text);
                let message = matrix_sdk::events::AnyMessageEventContent::RoomMessage(content);
                let client = client.clone();
                let id = id.clone();
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
        Key::Ctrl('r') => {}
        Key::Ctrl('s') => {
            if let Some(id) = &state.current_room_id {
                let client = client.clone();
                let prev_batch = client
                    .get_joined_room(&id)
                    .map(|r| r.last_prev_batch())
                    .unwrap_or(None)
                    .unwrap_or(String::new());
                let id = id.clone();
                tokio::task::spawn(async move {
                    let mut request =
                        matrix_sdk::api::r0::message::get_message_events::Request::backward(
                            &id,
                            &prev_batch,
                        );
                    request.limit = matrix_sdk::UInt::new(50).unwrap();
                    let r = client.room_messages(request).await.unwrap();
                    if let Some(prev_batch) = r.end {
                        tx.send(Event::Matrix(MatrixEvent::PrevBatch {
                            id: id.clone(),
                            prev_batch,
                        }))
                        .unwrap();
                    }
                    for event in r.chunk {
                        let event = match event.deserialize() {
                            Ok(e) => e,
                            Err(err) => {
                                crate::log::error(&err.to_string());
                                continue;
                            }
                        };
                        match event {
                            matrix_sdk::events::AnyRoomEvent::Message(m) => match m {
                                matrix_sdk::events::AnyMessageEvent::RoomMessage(message) => {
                                    tx.send(Event::Matrix(MatrixEvent::OldMessage {
                                        id: id.clone(),
                                        message: Message::new(
                                            message.sender,
                                            message.content,
                                            message.origin_server_ts,
                                        ),
                                    }))
                                    .unwrap();
                                }
                                _ => crate::log::info(&format!("{:?}\n", m)),
                            },
                            _ => crate::log::info(&format!("{:?}\n", event)),
                        }
                    }
                    crate::log::info("state\n");
                    for e in r.state {
                        crate::log::info(&format!("{:?}\n", e));
                    }
                    crate::log::info("\n\n");
                });
            }
        }
        // TODO still broken, needs to be pressed twice
        Key::Esc => return false,
        _ => {}
    };
    true
}

pub async fn handle_event(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
    state: &mut State,
    client: &mut matrix_sdk::Client,
    tx: &tokio::sync::mpsc::UnboundedSender<Event>,
) -> bool {
    let event = if let Some(e) = rx.recv().await {
        e
    } else {
        return false;
    };
    match event {
        Event::Keyboard(key) => handle_keyboard_event(key, state, client, tx.clone()),
        Event::Tick => true,
        Event::Matrix(e) => handle_matrix_event(e, state),
    }
}

fn handle_matrix_event(event: MatrixEvent, state: &mut State) -> bool {
    match event {
        MatrixEvent::RoomName { id, name } => match state.get_room_mut(&id) {
            Some(room) => room.name = name,
            None => state.rooms.push((id, Box::new(Room::new(name, 0)))),
        },
        MatrixEvent::NewMessage { id, message } => {
            if let Some(room) = state.get_room_mut(&id) {
                room.message_list.push_new(message)
            }
        }
        MatrixEvent::OldMessage { id, message } => {
            if let Some(room) = state.get_room_mut(&id) {
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
                .map(|room| room.prev_batch = prev_batch);
        }
    }
    true
}
