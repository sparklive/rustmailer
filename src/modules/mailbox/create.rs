// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    encode_mailbox_name, modules::account::v2::AccountV2,
    modules::context::executors::RUST_MAIL_CONTEXT, modules::error::RustMailerResult,
};

pub async fn create_mailbox(account_id: u64, mailbox_name: &str) -> RustMailerResult<()> {
    AccountV2::check_account_active(account_id).await?;
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    executor
        .create_mailbox(encode_mailbox_name!(mailbox_name).as_str())
        .await
}
