mod app;
mod config;
mod crypto;
mod events;
mod log;
mod matrix;
mod state;
mod ui;
mod utils;

use config::MientConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // tracing_subscriber::fmt::init();
    let home = std::env::var("HOME")?;
    let mut config_path = format!("{}/{}", &home, ".config/mient/config.json");
    let mut args: Vec<String> = Vec::new();
    for arg in std::env::args() {
        if arg.starts_with("--config=") {
            config_path = arg.strip_prefix("--config=").unwrap().to_string()
        } else {
            args.push(arg)
        }
    }
    let mient_config = config::MientConfig::get(&config_path)?;

    let client_config =
        matrix_sdk::ClientConfig::new().store_path(&format!("{}/{}", home, ".local/share/mient"));
    let homeserver_url = url::Url::parse(&mient_config.homeserver)
        .expect("Couldn't parse the homeserver URL, you might have forgotten to prefix https://");
    let mut client = matrix_sdk::Client::new_with_config(homeserver_url, client_config)?;

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
            crypto::verify_device(client, device).await?;
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
