use matrix_sdk::events::AnyToDeviceEvent;

pub async fn verify_device(
    client: matrix_sdk::Client,
    device: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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
    Ok(())
}
