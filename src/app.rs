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
    let (matrix_tx, mut matrix_rx) = tokio::sync::mpsc::unbounded_channel();
    let (mient_tx, mut mient_rx) = tokio::sync::mpsc::unbounded_channel();

    client
        .set_event_handler(Box::new(matrix::MatrixBroker::new(matrix_tx.clone())))
        .await;

    // SETUP LOCAL STATE
    let mut state = state::State::from_client(client.clone(), matrix_tx.clone()).await;

    // SETUP TERMINAL
    let stdout = std::io::stdout().into_raw_mode()?;
    let stdout = AlternateScreen::from(stdout);
    let backend = tui::backend::TermionBackend::new(stdout);
    let mut terminal = tui::Terminal::new(backend)?;

    // EVENT LOOP
    spawn_matrix_sync_task(client.clone(), matrix::MatrixBroker::new(matrix_tx.clone()));
    let input_handle = spawn_input_task(mient_tx.clone());
    spawn_tick_task(mient_tx.clone());

    let event_tx = matrix_tx.clone();
    loop {
        ui::draw(&mut terminal, &mut state)?;
        tokio::select! {
            event = mient_rx.recv() => {
                if !events::handle_mient_event(event.unwrap(), &mut state, &mut client, &event_tx).await {
                    break;
                }
            }
            event = matrix_rx.recv() => {
                events::handle_matrix_event(event.unwrap(), &mut state).await;
            }
        }
    }

    matrix_rx.close();
    mient_rx.close();
    let _ = tokio::join!(input_handle);
    drop(terminal);

    Ok(())
}

fn spawn_matrix_sync_task(
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
                matrix_broker.handle_sync_response(r).await
            })
            .await
    })
}

fn spawn_input_task(
    tx: tokio::sync::mpsc::UnboundedSender<events::MientEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        for event in std::io::stdin().keys() {
            if let Ok(key) = event {
                if let Err(_) = tx.send(events::MientEvent::Keyboard(key)) {
                    return;
                }
                if key == termion::event::Key::Esc {
                    return;
                }
            }
        }
    })
}

fn spawn_tick_task(
    tx: tokio::sync::mpsc::UnboundedSender<events::MientEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn(async move {
        loop {
            if let Err(_) = tx.send(events::MientEvent::Tick) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    })
}
