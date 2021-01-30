use matrix_sdk::api::r0::uiaa::AuthData;
use matrix_sdk::identifiers::UserId;
use serde_json::json;
use std::collections::BTreeMap;

use crate::config::MientConfig;

pub async fn cross_sign(
    mient_config: MientConfig,
    client: matrix_sdk::Client,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Err(e) = client.bootstrap_cross_signing(None).await {
        if let Some(response) = e.uiaa_response() {
            let auth_data = auth_data(
                &client.user_id().await.unwrap(),
                &mient_config.password,
                response.session.as_deref(),
            );
            client
                .bootstrap_cross_signing(Some(auth_data))
                .await
                .unwrap()
        } else {
            panic!("Error durign cross signing bootstrap {:#?}", e);
        }
    }

    Ok(())
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
