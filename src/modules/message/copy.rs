// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    encode_mailbox_name, modules::account::v2::AccountV2,
    modules::context::executors::RUST_MAIL_CONTEXT, modules::envelope::generate_uid_set,
    modules::error::RustMailerResult,
};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MailboxTransferRequest {
    /// A list of unique identifiers (UIDs) for the messages to be moved.
    pub uids: Vec<u32>,

    /// The name of the mailbox from which the messages will be moved.
    /// The decoded, human-readable name of the mailbox containing the email (e.g., "INBOX").
    /// This name is presented as it appears to users, with any encoding (e.g., UTF-7) automatically handled by the system,
    /// so no manual decoding is required.
    pub current_mailbox: String,

    /// The name of the mailbox to which the messages will be moved.
    /// The decoded, human-readable name of the mailbox containing the email (e.g., "INBOX").
    /// This name is presented as it appears to users, with any encoding (e.g., UTF-7) automatically handled by the system,
    /// so no manual decoding is required.
    pub target_mailbox: String,
}

pub async fn copy_mailbox_messages(
    account_id: u64,
    payload: &MailboxTransferRequest,
) -> RustMailerResult<()> {
    // Ensure the account exists before proceeding
    AccountV2::check_account_active(account_id).await?;

    // Generate a set of UIDs from the payload
    let uid_set = generate_uid_set(payload.uids.clone());

    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;

    // Encode the mailbox names using UTF-7 encoding
    let current_mailbox = encode_mailbox_name!(payload.current_mailbox.clone());
    let target_mailbox = encode_mailbox_name!(payload.target_mailbox.clone());

    // Move the messages from the current mailbox to the target mailbox
    executor
        .uid_copy_envelopes(
            uid_set.as_str(),
            current_mailbox.as_str(),
            target_mailbox.as_str(),
        )
        .await
}
