// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::Value;
use std::time::Duration;

use crate::rustmailer_version;

#[tokio::test]
async fn test1() {
    let access_token = "ya29.A0AS3H6Nx8aJrWEN2JG6zsSmhp_wg9BSS6i0uPoEbgPsdfdPtEydMK25ne2P0mL7FVu6g_F4rpTIDZxi4CO96LtlWokbTPd69vaVHx07qJfnRchq3lrFBLpm-OPqV0zLXbJ6X9VSsVW0Cd5W5ykSYjkQMXUA4u7iM7bpjps49VQo10_-y4l_FO7L28-Q0HGRCvdXJNRqgaaCgYKAVsSARUSFQHGX2Mi9nJ8LfBUMLimPBr_wEahxg0207";
    let url = "https://gmail.googleapis.com/gmail/v1/users/me/labels";
    let url = "https://gmail.googleapis.com/gmail/v1/users/me/labels/Label_6886728075529239043";
    let url = "https://gmail.googleapis.com/gmail/v1/users/me/messages?labelIds=INBOX&maxResults=10&pageToken=08792416985640480557";
    let url = "https://gmail.googleapis.com/gmail/v1/users/me/messages/198e590baf688394?format=metadata";
    let url = "https://gmail.googleapis.com/gmail/v1/users/me/messages?labelIds=INBOX&maxResults=10";
    let mut builder = reqwest::ClientBuilder::new()
        .user_agent(rustmailer_version!())
        .timeout(Duration::from_secs(10))
        .connect_timeout(Duration::from_secs(10));

    let proxy_obj = reqwest::Proxy::all("socks5://127.0.0.1:22308").unwrap();
    builder = builder
        .redirect(reqwest::redirect::Policy::none())
        .proxy(proxy_obj);
    let client = builder.build().unwrap();
    let res = client
        .get(url)
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await
        .unwrap();

    if res.status().is_success() {
        let body: Value = res.json().await.unwrap();
        let pretty = serde_json::to_string_pretty(&body).unwrap();
        println!("Response = {}", pretty);
    } else {
        eprintln!("Error: {} - {:?}", res.status(), res.text().await.unwrap());
    }
}
