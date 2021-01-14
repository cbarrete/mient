use matrix_sdk::identifiers::RoomId;
use termion::event::Key;

use crate::state::State;
use crate::state::Room;
use crate::state::Message;

#[derive(Debug)]
pub struct UserEvent;

#[derive(Debug)]
pub enum MatrixEvent {
    RoomName {
        id: RoomId,
        name: String,
    },
    Message {
        id: RoomId,
        message: Message,
    },
    Notifications {
        id: RoomId,
        count: matrix_sdk::UInt,
    }
}

#[derive(Debug)]
pub enum Event {
    Keyboard(Key),
    Debug(String),
    Matrix(MatrixEvent),
    Tick,
}

fn handle_keyboard_event(key: Key, state: &mut State, client: &mut matrix_sdk::Client) -> bool {
    match key {
        Key::Char('\n') => {
            if let Some(id) = &state.current_room_id {
                if state.input.is_empty() {
                    return true;
                }
                let text: String = state.input.drain(..).collect();
                let content = matrix_sdk::events::room::message::MessageEventContent::text_plain(text);
                let message = matrix_sdk::events::AnyMessageEventContent::RoomMessage(content);
                let client = client.clone();
                let id = id.clone();
                tokio::task::spawn(async move { client.room_send(&id, message, None).await }); // TODO txn id for local echo
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
                let id = id.clone();
                tokio::task::spawn(async move {
                    let token = client.sync_token().await.unwrap_or(String::new());
                    let mut request = matrix_sdk::api::r0::message::get_message_events::Request::backward(&id, &token);
                    request.limit = matrix_sdk::UInt::new(50).unwrap();
                    let r = client.room_messages(request).await;
                    println!("synced");
                    dbg!(r)
                });
            }
        }
        // TODO still broken, needs to be pressed twice
        Key::Esc => return false,
        _ => {}
    };
    true
}

pub async fn handle_event(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>,
                          state: &mut State,
                          client: &mut matrix_sdk::Client) -> bool {
    let event = if let Some(e) = rx.recv().await {
        e
    } else {
        return false;
    };
    match event {
        Event::Keyboard(key) => handle_keyboard_event(key, state, client),
        Event::Debug(message) => {
            state.debug_messages.push(message);
            true
        }
        Event::Tick => true,
        Event::Matrix(e) => handle_matrix_event(e, state),
    }
}

fn handle_matrix_event(event: MatrixEvent, state: &mut State) -> bool {
    match event {
        MatrixEvent::RoomName { id, name } => {
            match state.get_room_mut(&id) {
                Some(room) => room.name = name,
                None => state.rooms.push((id, Box::new(Room::new(name, matrix_sdk::UInt::MIN)))),
            }
        }
        MatrixEvent::Message { id, message } => {
            if let Some(room) = state.get_room_mut(&id) {
                room.add_message(message)
            }
        },
        MatrixEvent::Notifications{ id, count } => {
            state.get_room_mut(&id).map(|room| room.notifications = count );
        }
    }
    true
}
