use matrix_sdk::events::AnyToDeviceEvent;

mod app;
mod config;
mod events;
mod log;
mod matrix;
mod state;
mod ui;

use config::MientConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // tracing_subscriber::fmt::init();
    let home = std::env::var("HOME")?;
    let mient_config =
        config::MientConfig::get(&format!("{}/{}", &home, ".config/mient/config.json"))?;

    let client_config =
        matrix_sdk::ClientConfig::new().store_path(&format!("{}/{}", home, ".local/share/mient"));
    let homeserver_url = url::Url::parse(&mient_config.homeserver)
        .expect("Couldn't parse the homeserver URL, you might have forgotten to prefix https://");
    let mut client = matrix_sdk::Client::new_with_config(homeserver_url, client_config)?;

    let args: Vec<String> = std::env::args().collect();
    match args.iter().map(|s| s.as_str()).collect::<Vec<&str>>()[1..] {
        [] => {
            login(&mient_config, &mut client).await?;
            app::tui(client).await?
        }
        ["--import-keys", path, password] => {
            login(&mient_config, &mut client).await?;
            client
                .import_keys(path.into(), password)
                .await
                .map(|_| ())?
        }
        ["--export-keys", path, password] => {
            login(&mient_config, &mut client).await?;
            client.export_keys(path.into(), password, |_| true).await?
        }
        ["--list-devices"] => {
            login(&mient_config, &mut client).await?;
            let user_id = client.user_id().await.unwrap();
            for device in client.get_user_devices(&user_id).await?.devices() {
                println!(
                    "Device: {}, Trust: {:?}, Name: {}",
                    device.device_id(),
                    device.local_trust_state(),
                    device.display_name().as_deref().unwrap_or("")
                );
            }
        }
        ["--verify", device] => {
            login(&mient_config, &mut client).await?;
            let device = match client
                .get_device(&client.user_id().await.unwrap(), device.into())
                .await?
            {
                None => {
                    eprintln!("Device not found");
                    return Ok(()); // TODO should be an Err...
                }
                Some(d) => d,
            };
            if device.is_trusted() {
                println!("This device is already trusted");
                return Ok(());
            }

            println!("Starting verification...");
            let _ = device.start_verification().await?;

            client
                .sync_once(matrix_sdk::SyncSettings::new().full_state(true))
                .await?;
            let client = &client;
            client
                .sync_with_callback(matrix_sdk::SyncSettings::new(), |response| async move {
                    for event in response
                        .to_device
                        .events
                        .iter()
                        .filter_map(|e| e.deserialize().ok())
                    {
                        dbg!(&event);
                        match event {
                            AnyToDeviceEvent::KeyVerificationStart(e) => {
                                println!("Starting verification");
                                client
                                    .get_verification(&e.content.transaction_id)
                                    .await
                                    .unwrap()
                                    .accept()
                                    .await
                                    .unwrap();
                            }
                            AnyToDeviceEvent::KeyVerificationKey(e) => {
                                let sas = client
                                    .get_verification(&e.content.transaction_id)
                                    .await
                                    .unwrap();
                                println!("Emojis: {:?}", sas.emoji());
                                println!("Decimals: {:?}", sas.decimals());
                                println!("Do they match? (type yes if so) ");

                                let mut input = String::new();
                                std::io::stdin()
                                    .read_line(&mut input)
                                    .expect("error: unable to read user input");
                                if input.trim() == "yes" {
                                    println!("Confirming...");
                                    sas.confirm().await.unwrap();
                                    if sas.is_done() {
                                        println!("Done!");
                                        return matrix_sdk::LoopCtrl::Break;
                                    }
                                } else {
                                    println!("Aborting...");
                                    sas.cancel().await.unwrap();
                                }
                            }
                            AnyToDeviceEvent::KeyVerificationMac(e) => {
                                println!("Key verification mac");
                                let sas = client
                                    .get_verification(&e.content.transaction_id)
                                    .await
                                    .unwrap();
                                if sas.is_done() {
                                    println!("Done! Feel free to terminate the program, the following events aren't handled yet");
                                // return matrix_sdk::LoopCtrl::Break;
                                } else {
                                    println!("notdone..?");
                                }
                            }
                            AnyToDeviceEvent::KeyVerificationAccept(e) => {
                                println!("Accept");
                                let sas = client
                                    .get_verification(&e.content.transaction_id)
                                    .await
                                    .unwrap();
                                sas.accept().await.unwrap();
                            }
                            AnyToDeviceEvent::KeyVerificationCancel(e) => {
                                println!("They cancelled: {}", e.content.reason);
                                return matrix_sdk::LoopCtrl::Break;
                            }
                            e => {
                                dbg!(e);
                            }
                        }
                    }
                    matrix_sdk::LoopCtrl::Continue
                })
                .await;

            if device.is_trusted() {
                println!("Success!");
            } else {
                println!("The device is still untrusted..?");
            }
        }
        _ => usage(),
    }

    Ok(())
}

async fn login(
    mient_config: &MientConfig,
    client: &mut matrix_sdk::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Logging in...");
    client
        .login(
            &mient_config.user,
            &mient_config.password,
            Some(&mient_config.device_id),
            Some("mient"),
        )
        .await?;
    Ok(())
}

fn usage() {
    println!("Wrong arguments, must be either nothing or one of:");
    println!("--import-keys <file> <password>");
    println!("--export-keys <file> <password>");
    println!("--list-devices");
    println!("--verify <device>");
}
