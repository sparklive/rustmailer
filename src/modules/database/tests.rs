// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    id,
    modules::{
        account::{entity::AccountKey, v2::AccountV2},
        cache::{
            imap::{mailbox::MailBox, ENVELOPE_MODELS},
            vendor::gmail::sync::{envelope::GmailEnvelope, flow::max_history_id},
        },
        database::META_MODELS,
        hook::{
            entity::{EventHooks, HookType, HttpConfig, HttpMethod},
            events::EventType,
            payload::EventhookCreateRequest,
        },
    },
};
use itertools::Itertools;
use native_db::Builder;
use serde::{Deserialize, Serialize};

#[tokio::test]
async fn test2() {
    let all = MailBox::list_all(0u64).await.unwrap();
    for mailbox in all {
        println!("mailbox: {}", mailbox.id)
    }

    let all = AccountV2::minimal_list().await.unwrap();
    for mailbox in all {
        println!("account:{}", mailbox.id)
    }
}

#[test]
fn test3() {
    let database = Builder::new()
        .create(&META_MODELS, PathBuf::from("D://rustmailer_data//meta.db"))
        .unwrap();
    //database.compact().unwrap();
    let r_transaction = database.r_transaction().unwrap();
    let entities: Vec<AccountV2> = r_transaction
        .scan()
        .secondary(AccountKey::id)
        .unwrap()
        .all()
        .unwrap()
        .try_collect()
        .unwrap();
    println!("{:#?}", entities);

    let entities: Vec<AccountV2> = r_transaction
        .scan()
        .primary()
        .unwrap()
        .all()
        .unwrap()
        .try_collect()
        .unwrap();

    println!("{:#?}", entities);
}

#[tokio::test]
async fn test4() {
    let id = id!(64);
    let request = EventhookCreateRequest {
        account_id: Some(id),
        description: None,
        enabled: true,
        hook_type: HookType::Http,
        http: Some(HttpConfig {
            target_url: "http://localhost:15630".into(),
            http_method: HttpMethod::Post,
            custom_headers: BTreeMap::new(),
        }),
        nats: None,
        vrl_script: None,
        use_proxy: None,
        watched_events: vec![EventType::EmailSendingError],
    };
    let hook = EventHooks::new(request).await.unwrap();
    hook.save().await.unwrap();
    let hooks = EventHooks::get_by_account_id(id).await.unwrap();
    println!("{:#?}", hooks);
}

#[tokio::test]
async fn test5() {
    let mut account = AccountV2::default();
    let id = id!(64);
    account.id = id;

    account.save().await.unwrap();
    let account = AccountV2::get(id).await.unwrap();
    println!("{:#?}", account);
}

#[test]
fn test6() {
    #[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
    struct Test1 {
        f: u64,
    }

    let test = Test1 {
        f: 11259064003778907886,
    };

    println!("{}", serde_json::to_string_pretty(&test).unwrap());
}

#[test]
fn test7() {
    let database = Builder::new()
        .create(
            &ENVELOPE_MODELS,
            PathBuf::from("D://rustmailer_data//envelope.db"),
        )
        .unwrap();
    //database.compact().unwrap();
    let r_transaction = database.r_transaction().unwrap();
    let entities: Vec<GmailEnvelope> = r_transaction
        .scan()
        .primary()
        .unwrap()
        .all()
        .unwrap()
        .try_collect()
        .unwrap();
    // println!("{:#?}", entities);

    let history_ids: Vec<String> = entities
        .into_iter()
        .filter(|e| e.label_name == "INBOX")
        .map(|e| e.history_id)
        .collect();
    println!("{}", history_ids.len());

    let max_id = max_history_id(&history_ids);
    println!("{:#?}", max_id);
}
