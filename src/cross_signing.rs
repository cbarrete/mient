use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

use matrix_sdk::{api::r0::uiaa::AuthData, identifiers::UserId};
use serde_json::json;

fn auth_data<'a>(user: &UserId, password: &str, session: Option<&'a str>) -> AuthData<'a> {
    println!("ad");
    let mut auth_parameters = BTreeMap::new();
    let identifier = json!({
        "type": "m.id.user",
        "user": user,
    });

    auth_parameters.insert("identifier".to_owned(), identifier);
    auth_parameters.insert("password".to_owned(), password.to_owned().into());

    AuthData::DirectRequest {
        kind: "m.login.password",
        auth_parameters,
        session,
    }
}

async fn bootstrap(client: &matrix_sdk::Client, user_id: UserId, password: &str) {
    if let Err(e) = client.bootstrap_cross_signing(None).await {
        if let Some(response) = e.uiaa_response() {
            let auth_data = auth_data(&user_id, &password, response.session.as_deref());
            client
                .bootstrap_cross_signing(Some(auth_data))
                .await
                .expect("Couldn't bootstrap cross signing")
        } else {
            panic!("Error during cross-signing bootstrap {:#?}", e);
        }
    }
}

pub async fn cross_sign(
    client: &matrix_sdk::Client,
    password: &str,
) -> Result<(), matrix_sdk::Error> {
    client
        .sync_with_callback(matrix_sdk::SyncSettings::new(), |_| async move {
            let asked = AtomicBool::new(false);
            let client = client;
            let user_id = client.user_id().await.unwrap();

            // Wait for sync to be done then ask the user to bootstrap.
            if !asked.load(Ordering::SeqCst) {
                println!("bootstrapping...");
                bootstrap(client, user_id.clone(), password).await;
                println!("done!");
                return matrix_sdk::LoopCtrl::Break;
            }

            asked.store(true, Ordering::SeqCst);
            matrix_sdk::LoopCtrl::Continue
        })
        .await;

    Ok(())
}
