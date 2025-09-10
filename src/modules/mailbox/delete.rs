// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    encode_mailbox_name,
    modules::{
        account::{entity::MailerType, v2::AccountV2},
        cache::vendor::gmail::sync::client::GmailClient,
        context::executors::RUST_MAIL_CONTEXT,
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error,
};

pub async fn delete_mailbox(account_id: u64, mailbox_name: &str) -> RustMailerResult<()> {
    let account = AccountV2::check_account_active(account_id, false).await?;
    match account.mailer_type {
        MailerType::ImapSmtp => {
            let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
            executor
                .delete_mailbox(encode_mailbox_name!(mailbox_name).as_str())
                .await
        }
        MailerType::GmailApi => {
            let map = GmailClient::reverse_label_map(account_id, account.use_proxy, true).await?;
            let label_id = map.get(mailbox_name).ok_or_else(|| {
                raise_error!(
                    format!(
                        "Gmail label '{}' not found for account {}",
                        mailbox_name, account_id
                    ),
                    ErrorCode::ResourceNotFound
                )
            })?;
            GmailClient::delete_label(account_id, account.use_proxy, label_id).await
        }
    }
}
