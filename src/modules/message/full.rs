use crate::{
    modules::{
        account::entity::Account,
        cache::disk::DISK_CACHE,
        context::executors::RUST_MAIL_CONTEXT,
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error,
};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

use crate::modules::message::get_minimal_meta;

const MAX_EMAIL_TOTAL_SIZE: u32 = 55 * 1024 * 1024; // 55 MB

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct FullMessageRequest {
    pub mailbox: String,
    pub filename: String,
    pub uid: u32,
    pub max_length: Option<usize>,
}

fn full_email_diskcache_key(account_id: u64, mailbox_name: &str, uid: u32) -> String {
    format!("full_email_{}_{}_{}", account_id, mailbox_name, uid)
}

pub async fn retrieve_full_email(
    account_id: u64,
    mailbox: String,
    uid: u32,
) -> RustMailerResult<cacache::Reader> {
    Account::check_account_active(account_id).await?;
    let meta = get_minimal_meta(account_id, &mailbox, uid).await?;
    if meta.size > MAX_EMAIL_TOTAL_SIZE {
        return Err(raise_error!(format!(
            "Message size {} bytes exceeds maximum allowed size of {} bytes (UID: {}, Mailbox: '{}')",
            meta.size,
            MAX_EMAIL_TOTAL_SIZE,
            uid,
            mailbox
        ), ErrorCode::ExceedsLimitation));
    }

    let cache_key = full_email_diskcache_key(account_id, &mailbox, uid);
    if let Some(reader) = DISK_CACHE.get_cache(&cache_key).await? {
        return Ok(reader);
    }

    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    let fetch = executor
        .uid_fetch_full_message(uid.to_string().as_str(), &mailbox)
        .await?
        .ok_or_else(|| {
            raise_error!(
                format!("No message found for UID {} in mailbox {}", uid, &mailbox),
                ErrorCode::ImapUnexpectedResult
            )
        })?;

    let body = fetch.body().ok_or_else(|| {
        raise_error!(
            format!(
                "Message UID {} in mailbox {} is missing a body",
                uid, &mailbox
            ),
            ErrorCode::ImapUnexpectedResult
        )
    })?;

    DISK_CACHE.put_cache(&cache_key, body, false).await?;
    DISK_CACHE
        .get_cache(&cache_key)
        .await?
        .ok_or_else(|| raise_error!("Unexpected cache miss".into(), ErrorCode::InternalError))
}
