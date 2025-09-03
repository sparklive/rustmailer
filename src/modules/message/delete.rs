// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::encode_mailbox_name;
use crate::modules::account::v2::AccountV2;
use crate::modules::cache::imap::mailbox::{AttributeEnum, MailBox};
use crate::modules::context::executors::RUST_MAIL_CONTEXT;
use crate::modules::{envelope::generate_uid_set, error::RustMailerResult};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MessageDeleteRequest {
    /// A list of unique identifiers (UIDs) of the messages to be deleted.
    pub uids: Vec<u32>,

    /// The decoded, human-readable name of the mailbox containing the email (e.g., "INBOX").
    /// This name is presented as it appears to users, with any encoding (e.g., UTF-7) automatically handled by the system,
    /// so no manual decoding is required.
    pub mailbox: String,
}

pub async fn move_to_trash_or_delete_messages_directly(
    account_id: u64,
    request: &MessageDeleteRequest,
) -> RustMailerResult<()> {
    AccountV2::check_account_active(account_id, false).await?;
    let uid_set = generate_uid_set(request.uids.clone());
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
        || trash_or_junk_mailboxes
            .iter()
            .any(|m| m.name == request.mailbox)
    {
        let mailbox = encode_mailbox_name!(&request.mailbox);
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
        let from_mailbox_name = encode_mailbox_name!(&request.mailbox);
        executor
            .uid_move_envelopes(&uid_set, &from_mailbox_name, &to_mailbox_name)
            .await?;
    }

    Ok(())
}
