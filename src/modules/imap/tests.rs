use imap_proto::SectionPath;
use mail_parser::MessageParser;
use std::time::Instant;

use crate::modules::{
    bounce::parser::extract_bounce_report,
    imap::{
        executor::ImapExecutor,
        pool::build_imap_pool,
        section::{EmailBodyPart, Encoding, Param, PartType, SegmentPath},
    },
};

#[tokio::test]
async fn test1() {
    let pool = build_imap_pool(0u64).await.unwrap();
    let executor = ImapExecutor::new(pool);

    let a = executor
        .uid_fetch_full_message("18", "INBOX")
        .await
        .unwrap();
    if let Some(fetch) = a {
        // println!("{:#?}", &fetch.bodystructure());
        let content = fetch.body().unwrap();
        let message = MessageParser::new().parse(content).unwrap();
        let report = extract_bounce_report(&message);
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    }
}

#[tokio::test]
async fn test2() {
    let pool = build_imap_pool(0u64).await.unwrap();
    let executor = ImapExecutor::new(pool);

    let a = executor
        .uid_search("INBOX", "SUBJECT \"Jobstreet\"")
        .await
        .unwrap();
    println!("uid:{:#?}", a);
}

#[tokio::test]
async fn test3() {
    let pool = build_imap_pool(0u64).await.unwrap();
    let executor = ImapExecutor::new(pool);
    let start = Instant::now();
    let a = executor
        .uid_fetch_single_part("1", "Notification", "1")
        .await
        .unwrap();

    let first = a.first().unwrap();
    let part = EmailBodyPart::new(
        PartType::Plain,
        SegmentPath::new(vec![]),
        Some(vec![Param {
            key: "charset".into(),
            value: "UTF-8".into(),
        }]),
        546,
        Encoding::None,
    );

    let result = first.section(&SectionPath::Part(vec![1], None));

    let content: String = match std::str::from_utf8(result.unwrap()) {
        Ok(valid_str) => valid_str.into(),
        Err(_) => "???".into(),
    };
    println!("content:{}", content);
}
