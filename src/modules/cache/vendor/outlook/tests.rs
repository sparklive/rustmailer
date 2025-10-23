// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::{future::Future, pin::Pin};

use graph_rs_sdk::GraphClient;
use poem_grpc::{ClientConfig, CompressionEncoding};
use reqwest::header::AUTHORIZATION;

use crate::{
    modules::{
        cache::vendor::outlook::model::{MailFolder, MailFoldersResponse},
        common::rustls::RustMailerTls,
        context::Initialize,
        error::{code::ErrorCode, RustMailerResult},
        grpc::service::rustmailer_grpc::{GetOAuth2TokensRequest, OAuth2ServiceClient},
    },
    raise_error,
};

async fn access_token() -> String {
    RustMailerTls::initialize().await.unwrap();

    let cfg = ClientConfig::builder()
        .uri("http://localhost:16630")
        .build()
        .unwrap();
    let mut grpc_client = OAuth2ServiceClient::new(cfg);
    grpc_client.set_accept_compressed([CompressionEncoding::GZIP]);
    grpc_client.set_send_compressed(CompressionEncoding::GZIP);

    let request = GetOAuth2TokensRequest {
        account_id: 711146144129468,
    };

    let mut request = poem_grpc::Request::new(request);
    request.metadata_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );
    let result = grpc_client.get_o_auth2_tokens(request).await.unwrap();
    result.access_token.clone().unwrap()
}

#[tokio::test]
async fn test1() {
    let access_token = access_token().await;
    let client = GraphClient::new(&access_token);
    let folders = client
        .me()
        .mail_folders()
        .list_mail_folders()
        .send()
        .await
        .unwrap();
    let json: MailFoldersResponse = folders.json().await.unwrap();
    println!("{:#?}", json);
}

#[tokio::test]
async fn test2() {
    let access_token = access_token().await;
    let client = GraphClient::new(&access_token);
    let folders = client
        .me()
        .mail_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtUAAAA=")
        .child_folders().list_child_folders()
        .send()
        .await
        .unwrap();
    let json: MailFoldersResponse = folders.json().await.unwrap();
    println!("{:#?}", json);
}

fn fetch_recursive<'a>(
    client: &'a GraphClient,
    folder_id: Option<&'a str>,
    prefix: &'a str,
    output: &'a mut Vec<MailFolder>,
) -> Pin<Box<dyn Future<Output = RustMailerResult<()>> + 'a>> {
    Box::pin(async move {
        let folders_response: MailFoldersResponse = match folder_id {
            Some(id) => client
                .me()
                .mail_folder(id)
                .child_folders()
                .list_child_folders()
                .send()
                .await
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .json::<MailFoldersResponse>()
                .await
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?,
            None => client
                .me()
                .mail_folders()
                .list_mail_folders()
                .send()
                .await
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .json::<MailFoldersResponse>()
                .await
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?,
        };

        for mut folder in folders_response.value {
            let full_name = if prefix.is_empty() {
                folder.display_name.clone()
            } else {
                format!("{}/{}", prefix, folder.display_name)
            };
            folder.display_name = full_name.clone();
            output.push(folder.clone());

            if folder.child_folder_count.unwrap_or(0) > 0 {
                fetch_recursive(client, Some(&folder.id), &full_name, output).await?;
            }
        }
        Ok(())
    })
}

pub async fn fetch_flattened_mailfolders(
    client: &GraphClient,
) -> RustMailerResult<Vec<MailFolder>> {
    let mut result = Vec::new();
    fetch_recursive(client, None, "", &mut result).await?;
    Ok(result)
}

#[tokio::test]
async fn test3() {
    let access_token = access_token().await;
    let client = GraphClient::new(&access_token);
    let result = fetch_flattened_mailfolders(&client).await.unwrap();
    println!("{:#?}", result);
}

#[tokio::test]
async fn test4() {
    let access_token = access_token().await;
    let client = GraphClient::new(&access_token);
    let response = client
        .me()
        .mail_folders()
        .create_mail_folders(&serde_json::json!({
            "displayName": "test2"
        }))
        .send()
        .await
        .unwrap();
    let response = client
        .me()
        .mail_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtUAAAA=")
        .child_folders()
        .create_child_folders(&serde_json::json!({
            "displayName": "test12"
        }))
        .send()
        .await.unwrap();
}

#[tokio::test]
async fn test5() {
    let access_token = access_token().await;
    let client = GraphClient::new(&access_token);
    
    // let response = client
    //     .me()
    //     .mail_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtUAAAA=")
    //     .child_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtsAAAA=")
    //     .delete_child_folders()
    //     .send()
    //     .await.unwrap();
    
    let response = client
        .me()
        .mail_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtUAAAA=")
        .delete_mail_folders()
        .send()
        .await.unwrap();
}

#[tokio::test]
async fn test6() {
    let access_token = access_token().await;
    let client = GraphClient::new(&access_token);
    let response = client
        .me()
        .mail_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtUAAAA=")
        .update_mail_folders(&serde_json::json!({
            "displayName": "new_name"
        }))
        .send()
        .await.unwrap();

    let response = client
        .me()
        .mail_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtUAAAA=")
        .child_folder("AQMkADAwATMwMAItNzE0OC1jZTEzLTAwAi0wMAoALgAAA_KUk7xWPSBEntPHShr61lgBAOo9V4GwHndCjf0x1uoIcwUAAYiJQtsAAAA=")
        .update_child_folders(&serde_json::json!({
            "displayName": "new_subfolder_name"
        }))
        .send()
        .await
        .unwrap();
}
