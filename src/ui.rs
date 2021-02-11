use std::collections::{HashMap, VecDeque};

use matrix_sdk::identifiers::UserId;
use tui::backend::Backend;
use tui::style::Color;
use tui::style::Modifier;
use tui::style::Style;
use tui::text::Text;
use tui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
};
use unicode_width::UnicodeWidthStr;

use crate::state::Message;
use crate::state::Room;
use crate::state::State;

struct MientLayout {
    rooms_region: Rect,
    messages_region: Rect,
    input_region: Rect,
}

fn make_layout(terminal_size: Rect) -> MientLayout {
    let main_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(25), Constraint::Min(1)]) // TODO maybe configurable or resizable
        .split(terminal_size);

    let right_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(main_layout[1]);

    MientLayout {
        rooms_region: main_layout[0],
        messages_region: right_layout[0],
        input_region: right_layout[1],
    }
}

// TODO could return some fancy stuff for formatting
fn format_message(message: &Message, users: &HashMap<UserId, String>) -> String {
    let sender = if let Some(sender) = users.get(&message.sender) {
        sender
    } else {
        message.sender.localpart()
    };
    let body = match &message.content {
        matrix_sdk::events::room::message::MessageEventContent::Text(content) => {
            content.body.clone()
        }
        other => format!("{:?}", other),
    };
    format!("{}: {}", sender, body)
}

fn format_room_name(room: &Room) -> tui::text::Text {
    if room.notifications > 0 {
        let style = Style::default().fg(Color::Red);
        Text::styled(&room.name, style)
    } else {
        Text::from(room.name.as_ref())
    }
}

pub fn draw<T: Backend>(terminal: &mut Terminal<T>, state: &mut State) -> std::io::Result<()> {
    terminal.draw(|f| {
        let layout = make_layout(f.size());

        let mut rooms: Vec<ListItem> = Vec::with_capacity(state.rooms.len());
        let mut selected = None;
        for (i, (room_id, room)) in state.rooms.iter().enumerate() {
            if Some(room_id.clone()) == state.current_room_id {
                selected = Some(i)
            }
            rooms.push(ListItem::new(format_room_name(&room)));
        }
        let room_list = List::new(rooms)
            .block(Block::default().borders(Borders::RIGHT))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .highlight_symbol(">");
        let mut room_list_state = ListState::default();
        room_list_state.select(selected);
        f.render_stateful_widget(room_list, layout.rooms_region, &mut room_list_state);

        let messages: Vec<ListItem> = state
            .get_current_room()
            .map(|room| &room.message_list.messages)
            .unwrap_or(&VecDeque::new())
            .iter()
            .map(|message| ListItem::new(format_message(message, &state.users)))
            .collect();
        let index = if messages.len() > 0 {
            messages.len() - 1
        } else {
            0
        };
        let message_list = List::new(messages).block(Block::default().borders(Borders::BOTTOM));
        room_list_state.select(Some(index));
        f.render_stateful_widget(message_list, layout.messages_region, &mut room_list_state);

        let is = state.input.width() as u16;
        let rs = layout.input_region.width;
        let input = Paragraph::new(state.input.as_ref())
            .scroll((0, if is + 1 > rs { is + 1 - rs } else { 0 }));
        f.render_widget(input, layout.input_region);

        f.set_cursor(
            layout.input_region.x + state.input.width() as u16,
            layout.input_region.y,
        );
    })
}
