// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::message::copy::MailboxTransferRequest;
use crate::{
    encode_mailbox_name, modules::account::v2::AccountV2,
    modules::context::executors::RUST_MAIL_CONTEXT, modules::envelope::generate_uid_set,
    modules::error::RustMailerResult,
};

pub async fn move_mailbox_messages(
    account_id: u64,
    payload: &MailboxTransferRequest,
) -> RustMailerResult<()> {
    // Ensure the account exists before proceeding
    AccountV2::check_account_active(account_id).await?;

    // Generate a set of UIDs from the payload
    let uid_set = generate_uid_set(payload.uids.clone());
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;

    // Encode the mailbox names using UTF-7 encoding
    let current_mailbox = encode_mailbox_name!(&payload.current_mailbox);
    let target_mailbox = encode_mailbox_name!(&payload.target_mailbox);

    // Move the messages from the current mailbox to the target mailbox
    executor
        .uid_move_envelopes(
            uid_set.as_str(),
            current_mailbox.as_str(),
            target_mailbox.as_str(),
        )
        .await
}
