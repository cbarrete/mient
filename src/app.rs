use matrix_sdk;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::screen::AlternateScreen;

use crate::events;
use crate::matrix;
use crate::state;
use crate::ui;

pub async fn tui(mut client: matrix_sdk::Client) -> Result<(), Box<dyn std::error::Error>> {
    // SETUP COMMUNICATION
    println!("Setting up communication");
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    client
        .add_event_emitter(Box::new(matrix::MatrixBroker::new(tx.clone())))
        .await;

    // SETUP LOCAL STATE
    let mut state = state::State::new();
    state.populate(&mut client).await;

    // SETUP TERMINAL
    let stdout = std::io::stdout().into_raw_mode()?;
    let stdout = AlternateScreen::from(stdout);
    let backend = tui::backend::TermionBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;

    // EVENT LOOP
    spawn_matrix_task(client.clone(), matrix::MatrixBroker::new(tx.clone()));
    let input_handle = spawn_input_task(tx.clone());
    spawn_tick_task(tx.clone());

    let event_tx = tx.clone();

    loop {
        ui::draw(&mut terminal, &mut state)?;
        // TODO passing that event_tx probably isn't the cleanest way to do that, I probably want a
        // struct that owns a MatrixBroker and handles matrix operations (sending, requesting old
        // messages, etc.)
        if !events::handle_event(&mut rx, &mut state, &mut client, &event_tx).await {
            break;
        }
    }

    for id in client.joined_rooms().read().await.keys() {
        let client = client.clone();
        let id = id.clone();
        tokio::task::spawn(async move { client.store_room_state(&id).await });
    }

    rx.close();
    let _ = tokio::join!(input_handle);
    drop(terminal);

    Ok(())
}

fn spawn_matrix_task(
    client: matrix_sdk::Client,
    matrix_broker: matrix::MatrixBroker,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        let mut sync_settings = matrix_sdk::SyncSettings::new();
        if let Some(token) = client.sync_token().await {
            sync_settings = sync_settings.token(token);
        }
        client
            .sync_with_callback(sync_settings, |r| async {
                matrix_broker.handle_response(r).await
            })
            .await
    })
}

fn spawn_input_task(
    tx: tokio::sync::mpsc::UnboundedSender<events::Event>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        for event in std::io::stdin().keys() {
            if let Ok(key) = event {
                if let Err(_) = tx.send(events::Event::Keyboard(key)) {
                    return;
                }
            }
        }
    })
}

fn spawn_tick_task(
    tx: tokio::sync::mpsc::UnboundedSender<events::Event>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        loop {
            if let Err(_) = tx.send(events::Event::Tick) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    })
}
