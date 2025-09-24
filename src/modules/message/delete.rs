// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::account::entity::MailerType;
use crate::modules::account::v2::AccountV2;
use crate::modules::cache::imap::mailbox::{AttributeEnum, MailBox};
use crate::modules::cache::vendor::gmail::sync::client::GmailClient;
use crate::modules::context::executors::RUST_MAIL_CONTEXT;
use crate::modules::error::code::ErrorCode;
use crate::modules::{envelope::generate_uid_set, error::RustMailerResult};
use crate::{encode_mailbox_name, raise_error};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MessageDeleteRequest {
    /// A list of unique identifiers (UIDs) of the messages to be deleted (IMAP only).
    pub uids: Option<Vec<u32>>,
    /// A list of Gmail message IDs of the messages to be deleted (Gmail API only).
    pub mids: Option<Vec<String>>,
    /// The decoded, human-readable name of the mailbox containing the email (e.g., "INBOX").  (IMAP only)
    /// This name is presented as it appears to users, with any encoding (e.g., UTF-7) automatically handled by the system,
    /// so no manual decoding is required.
    /// In Gmail API, this field is not required and can be set to `None`.
    pub mailbox: Option<String>,
}

pub async fn move_to_trash(
    account_id: u64,
    request: &MessageDeleteRequest,
) -> RustMailerResult<()> {
    let account = AccountV2::check_account_active(account_id, false).await?;

    match account.mailer_type {
        MailerType::ImapSmtp => {
            let mailbox = request.mailbox.as_deref().ok_or_else(|| {
                raise_error!(
                    "IMAP request missing required field 'mailbox'".into(),
                    ErrorCode::InvalidParameter
                )
            })?;

            let uids = request.uids.as_deref().ok_or_else(|| {
                raise_error!(
                    "IMAP request missing required field 'uids'".into(),
                    ErrorCode::InvalidParameter
                )
            })?;

            move_to_trash_or_delete_messages_directly(account_id, uids, mailbox).await
        }
        MailerType::GmailApi => {
            let mids = request.mids.as_deref().ok_or_else(|| {
                raise_error!(
                    "Gmail request missing required field 'mids'".into(),
                    ErrorCode::InvalidParameter
                )
            })?;
            gmail_move_to_trash(&account, mids).await
        }
    }
}

pub async fn gmail_move_to_trash(account: &AccountV2, mids: &[String]) -> RustMailerResult<()> {
    let account_id = account.id;
    let use_proxy = account.use_proxy;
    GmailClient::batch_delete(account_id, use_proxy, mids).await
}

async fn move_to_trash_or_delete_messages_directly(
    account_id: u64,
    uids: &[u32],
    mailbox: &str,
) -> RustMailerResult<()> {
    let uid_set = generate_uid_set(uids.to_vec());
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;

    let all_mailboxes: Vec<MailBox> = executor
        .list_all_mailboxes()
        .await?
        .iter()
        .map(|name| name.into())
        .collect();

    let trash_or_junk_mailboxes: Vec<&MailBox> = all_mailboxes
        .iter()
        .filter(|mailbox| {
            mailbox.has_attr(&AttributeEnum::Trash) || mailbox.has_attr(&AttributeEnum::Junk)
        })
        .collect();

    if trash_or_junk_mailboxes.is_empty()
        || trash_or_junk_mailboxes.iter().any(|m| m.name == mailbox)
    {
        let mailbox = encode_mailbox_name!(mailbox);
        executor
            .uid_delete_envelopes(uid_set.as_str(), mailbox.as_str())
            .await?;
        return Ok(());
    }

    let trash_first_target = all_mailboxes
        .iter()
        .find(|mailbox| mailbox.has_attr(&AttributeEnum::Trash))
        // If no trash is found, try to find the mailbox marked as junk
        .or_else(|| {
            all_mailboxes
                .iter()
                .find(|mailbox| mailbox.has_attr(&AttributeEnum::Junk))
        });

    if let Some(target_mailbox) = trash_first_target {
        let to_mailbox_name = encode_mailbox_name!(&target_mailbox.name);
        let from_mailbox_name = encode_mailbox_name!(mailbox);
        executor
            .uid_move_envelopes(&uid_set, &from_mailbox_name, &to_mailbox_name)
            .await?;
    }

    Ok(())
}
