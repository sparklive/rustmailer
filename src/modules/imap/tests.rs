// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::{account::entity::Encryption, imap::client::Client};

#[tokio::test]
async fn testxx() {
    rustls::crypto::CryptoProvider::install_default(rustls::crypto::ring::default_provider())
        .unwrap();
    let client = Client::connection("imap.zoho.com".into(), Encryption::Ssl, 993, None)
        .await
        .unwrap();
    let mut session = client
        .login("pollybase@zohomail.com", "xxx")
        .await
        .unwrap();
    session.select("INBOX").await.unwrap();
    let result = session
        .uid_search("LARGER 1024")
        .await
        .unwrap();
    println!("{:#?}", result);
}
 