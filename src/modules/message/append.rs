// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::borrow::Cow;

use mail_send::{
    mail_builder::{headers::address::Address, MessageBuilder},
    smtp::message::IntoMessage,
};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

use crate::{
    encode_mailbox_name,
    modules::{
        account::entity::Account,
        context::executors::RUST_MAIL_CONTEXT,
        error::{code::ErrorCode, RustMailerResult},
        smtp::request::{reply::apply_references, EmailHandler},
    },
    raise_error,
};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct AppendReplyToDraftRequest {
    /// The name of the mailbox containing the original message.
    ///
    /// This is used to locate the source message that is being replied to.
    pub mailbox_name: String,
    /// The UID of the message being replied to.
    ///
    /// This identifies the specific message in the mailbox.
    pub uid: u32,
    /// A preview text for the reply email.
    ///
    /// This optional field provides a short summary or preview of the reply content.
    pub preview: Option<String>,
    /// The plain text body of the reply email.
    ///
    /// This field is optional and can be used to provide plain text content.
    pub text: Option<String>,
    /// The HTML body of the reply email.
    ///
    /// This field is optional and can be used to provide HTML content.
    pub html: Option<String>,
    // For example: "[Gmail]/Drafts"
    // This can be obtained from the `name` field of the mailbox via the list-mailboxes endpoint.
    pub draft_folder_path: String,
}

impl AppendReplyToDraftRequest {
    pub async fn append_reply_to_draft(&self, account_id: u64) -> RustMailerResult<()> {
        let account = Account::check_account_active(account_id).await?;
        let envelope = EmailHandler::get_envelope(&account, &self.mailbox_name, self.uid).await?;
        let from = Address::new_address(
            account.name.as_ref().map(|n| Cow::Owned(n.to_string())),
            Cow::Owned(account.email.clone()),
        );
        let to = match &envelope.reply_to {
            Some(reply_to) if !reply_to.is_empty() => reply_to.clone(),
            _ => envelope
                .from
                .clone()
                .map(|from| vec![from])
                .ok_or_else(|| {
                    raise_error!(
                        "Invalid email envelope: missing both 'reply_to' and 'from'".into(),
                        ErrorCode::InvalidParameter
                    )
                })?,
        };
        let subject = format!("Re: {}", envelope.subject.as_deref().unwrap_or(""));
        let mut builder = MessageBuilder::new()
            .from(from)
            .to(Address::from(to.clone()))
            .subject(subject.clone());
        builder = apply_references(builder, &envelope)?;
        builder = self.apply_content(builder)?;
        let message = builder.into_message().map_err(|e| {
            raise_error!(
                format!("Failed to build message: {}", e),
                ErrorCode::InternalError
            )
        })?;
        let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
        let drafts = encode_mailbox_name!(&self.draft_folder_path);
        executor.append(drafts, None, None, message.body).await
    }

    fn apply_content(
        &self,
        mut builder: MessageBuilder<'static>,
    ) -> RustMailerResult<MessageBuilder<'static>> {
        if let Some(html) = &self.html {
            let html = EmailHandler::insert_preview(&self.preview, html.clone());
            builder = builder.html_body(html);
        }
        if let Some(text) = &self.text {
            builder = builder.text_body(text.clone());
        }
        Ok(builder)
    }
}
