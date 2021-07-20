use matrix_sdk::{
    ruma::events::{
        key::verification::{
            key::KeyToDeviceEventContent, mac::MacToDeviceEventContent,
            start::StartToDeviceEventContent,
        },
        AnyToDeviceEvent, ToDeviceEvent,
    },
    verification::Verification,
};

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
    if device.is_locally_trusted() {
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
                        start_verification(e, client).await;
                    }
                    AnyToDeviceEvent::KeyVerificationKey(e) => {
                        verify_key(e, client).await;
                    }
                    AnyToDeviceEvent::KeyVerificationMac(e) => {
                        verify_mac(e, client).await;
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

    if device.is_locally_trusted() {
        println!("Success!");
    } else {
        println!("The device is still untrusted..?");
    }
    Ok(())
}

async fn verify_mac(e: ToDeviceEvent<MacToDeviceEventContent>, client: &matrix_sdk::Client) -> () {
    println!("Key MAC verification");
    if let Some(Verification::SasV1(sas)) = client
        .get_verification(&e.sender, &e.content.transaction_id)
        .await
    {
        if sas.is_done() {
            println!(
                "Done! Feel free to terminate the program, the following events aren't handled yet"
            );
        } else {
            println!("notdone..?");
        }
    }
}

async fn verify_key(e: ToDeviceEvent<KeyToDeviceEventContent>, client: &matrix_sdk::Client) -> () {
    if let Some(Verification::SasV1(sas)) = client
        .get_verification(&e.sender, &e.content.transaction_id)
        .await
    {
        println!("Emojis: {:?}", sas.emoji());
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
            } else {
                println!("Error: verification process is still not done but quitting");
                // TODO if get here, only break if done
            }
        } else {
            println!("Aborting...");
            sas.cancel().await.unwrap();
        }
    }
}

async fn start_verification(
    e: ToDeviceEvent<StartToDeviceEventContent>,
    client: &matrix_sdk::Client,
) -> () {
    if let Some(Verification::SasV1(sas)) = client
        .get_verification(&e.sender, &e.content.transaction_id)
        .await
    {
        println!(
            "Starting verification with {} {}",
            &sas.other_device().user_id(),
            &sas.other_device().device_id()
        );
        // print_devices(&e.sender, client).await;
        sas.accept().await.unwrap();
    }
}
