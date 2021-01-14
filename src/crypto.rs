use matrix_sdk::api::r0::uiaa::AuthData;
use matrix_sdk::identifiers::UserId;
use serde_json::json;
use std::collections::BTreeMap;

use crate::config::MientConfig;

pub async fn cross_sign(mient_config: MientConfig, client: matrix_sdk::Client) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = client.bootstrap_cross_signing(None).await {
        if let Some(response) = e.uiaa_response() {
            let auth_data = auth_data(&client.user_id().await.unwrap(), &mient_config.password, response.session.as_deref());
            client
                .bootstrap_cross_signing(Some(auth_data))
                .await
                .expect("Couldn't bootstrap cross signing")
        } else {
            panic!("Error durign cross signing bootstrap {:#?}", e);
        }
    }

    Ok(())

    // let devices = client.get_user_devices(&login.user_id).await.unwrap();
    // for device in devices.devices() {
    //     // device.set_local_trust(matrix_sdk::LocalTrust::Verified).await?;
    //     println!("{:?}", device);
    // }


    // let device = client.get_device(dbg!(&login.user_id), dbg!(&login.device_id))
    //     .await
    //     .unwrap()
    //     .unwrap();

    // println!("is trusted {}", device.is_trusted());

    // let verification = device.start_verification().await.unwrap();

    // println!("decimals: {:?}", verification.decimals().unwrap());

    // println!("is trusted {}", device.is_trusted());

    // return Ok(());
}

fn auth_data<'a>(user: &UserId, password: &str, session: Option<&'a str>) -> AuthData<'a> {
    let mut auth_parameters = BTreeMap::new();
    let identifier = json!({
        "type": "m.id.user",
        "user": user,
    });

    auth_parameters.insert("identifier".to_owned(), identifier);
    auth_parameters.insert("password".to_owned(), password.to_owned().into());

    auth_parameters.insert("user".to_owned(), user.as_str().into());

    AuthData::DirectRequest {
        kind: "m.login.password",
        auth_parameters,
        session,
    }
}
