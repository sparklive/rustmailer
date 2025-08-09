// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use http::header::AUTHORIZATION;
use poem_grpc::{ClientConfig, CompressionEncoding, Metadata};

use crate::{
    id,
    modules::{
        common::rustls::RustMailerTls,
        context::Initialize,
        grpc::service::rustmailer_grpc::{
            GetThreadMessagesRequest, ListMessagesRequest, ListThreadsRequest,
            MessageServiceClient, TemplateSentTestRequest, TemplatesServiceClient,
            UnifiedSearchRequest,
        },
    },
};

#[tokio::test]
async fn test1() {
    let cfg = ClientConfig::builder()
        .uri("https://localhost:16630")
        .build()
        .unwrap();
    let mut grpc_client = MessageServiceClient::new(cfg);
    grpc_client.set_accept_compressed([CompressionEncoding::GZIP]);
    grpc_client.set_send_compressed(CompressionEncoding::GZIP);

    let mut metadata = Metadata::new();
    metadata.insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );

    let request = ListMessagesRequest {
        account_id: id!(64),
        mailbox_name: "INBOX".into(),
        page: 1,
        page_size: 10,
        remote: false,
        desc: true,
    };

    let mut request = poem_grpc::Request::new(request);
    request.metadata_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );

    let response = grpc_client.list_messages(request).await.unwrap();

    let paginated = response.into_inner();
    println!("{:#?}", paginated);
}

#[tokio::test]
async fn test2() {
    RustMailerTls::initialize().await.unwrap();

    let cfg = ClientConfig::builder()
        .uri("http://localhost:16630")
        .build()
        .unwrap();
    let mut grpc_client = TemplatesServiceClient::new(cfg);
    grpc_client.set_accept_compressed([CompressionEncoding::GZIP]);
    grpc_client.set_send_compressed(CompressionEncoding::GZIP);

    let request = TemplateSentTestRequest {
        template_id: 5817286801634245,
        account_id: 5737460794141278,
        recipient: "pollybase@zohomail.com".to_string(),
        template_params: None,
    };

    let mut request = poem_grpc::Request::new(request);
    request.metadata_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );

    let response = grpc_client.send_test_email(request).await.unwrap();
}

#[tokio::test]

async fn test3() {
    RustMailerTls::initialize().await.unwrap();

    let cfg = ClientConfig::builder()
        .uri("http://localhost:16630")
        .build()
        .unwrap();
    let mut grpc_client = MessageServiceClient::new(cfg);
    grpc_client.set_accept_compressed([CompressionEncoding::GZIP]);
    grpc_client.set_send_compressed(CompressionEncoding::GZIP);

    let request = UnifiedSearchRequest {
        accounts: Vec::new(),
        email: "no-reply@accounts.google.com".into(),
        after: None,
        before: None,
        page: 1,
        page_size: 15,
        desc: true,
    };

    let mut request = poem_grpc::Request::new(request);
    request.metadata_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );
    let response = grpc_client.unified_search(request).await.unwrap();
    println!("{:#?}", response.items);
}

#[tokio::test]
async fn test4() {
    RustMailerTls::initialize().await.unwrap();

    let cfg = ClientConfig::builder()
        .uri("http://localhost:16630")
        .build()
        .unwrap();
    let mut grpc_client = MessageServiceClient::new(cfg);
    grpc_client.set_accept_compressed([CompressionEncoding::GZIP]);
    grpc_client.set_send_compressed(CompressionEncoding::GZIP);

    let request = ListThreadsRequest {
        account_id: 8869750310191797,
        mailbox_name: "INBOX".into(),
        page: 1,
        page_size: 15,
        desc: true,
    };

    let mut request = poem_grpc::Request::new(request);
    request.metadata_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );
    let response = grpc_client.list_threads(request).await.unwrap();
    println!("{:#?}", response.items);
}

#[tokio::test]
async fn test5() {
    RustMailerTls::initialize().await.unwrap();

    let cfg = ClientConfig::builder()
        .uri("http://localhost:16630")
        .build()
        .unwrap();
    let mut grpc_client = MessageServiceClient::new(cfg);
    grpc_client.set_accept_compressed([CompressionEncoding::GZIP]);
    grpc_client.set_send_compressed(CompressionEncoding::GZIP);

    let request = GetThreadMessagesRequest {
        account_id: 6606017263301165,
        mailbox_name: "INBOX".into(),
        thread_id: 1572863359614161,
    };

    let mut request = poem_grpc::Request::new(request);
    request.metadata_mut().insert(
        AUTHORIZATION,
        format!("Bearer {}", "2mY4irNCahQXeSarHYje1P1W"),
    );
    let response = grpc_client.get_thread_messages(request).await.unwrap();
    println!("{:#?}", response.items);
}
