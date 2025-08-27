// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    encode_mailbox_name,
    modules::{
        account::v2::AccountV2, context::executors::RUST_MAIL_CONTEXT, error::RustMailerResult,
    },
};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MailboxRenameRequest {
    /// The current name of the mailbox
    #[oai(validator(min_length = "1", max_length = "1024"))]
    pub current_name: String,
    /// The new name for the mailbox
    #[oai(validator(min_length = "1", max_length = "1024"))]
    pub new_name: String,
}

pub async fn rename_mailbox(
    account_id: u64,
    payload: MailboxRenameRequest,
) -> RustMailerResult<()> {
    AccountV2::check_account_active(account_id).await?;
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    executor
        .rename_mailbox(
            encode_mailbox_name!(&payload.current_name).as_str(),
            encode_mailbox_name!(&payload.new_name).as_str(),
        )
        .await
}
