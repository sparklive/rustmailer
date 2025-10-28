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
use serde_json::json;

use crate::{
    base64_encode_url_safe, encode_mailbox_name,
    modules::{
        account::{entity::MailerType, migration::AccountModel},
        cache::vendor::gmail::sync::{
            client::GmailClient, envelope::GmailEnvelope, labels::GmailLabels,
        },
        context::executors::RUST_MAIL_CONTEXT,
        error::{code::ErrorCode, RustMailerResult},
        smtp::request::{
            reply::{apply_references, apply_references2},
            EmailHandler,
        },
    },
    raise_error,
};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct AppendReplyToDraftRequest {
    /// The name of the mailbox or label containing the original message.
    ///
    /// - For IMAP accounts, this is the mailbox name where the source message resides.
    /// - For Gmail API accounts, this refers to the label name associated with the source message.
    /// This is used to locate the message being replied to.
    pub mailbox_name: String,
    /// The unique ID of the message, either IMAP UID or Gmail API MID.
    ///
    /// - For IMAP accounts, this is the UID converted to a string. It must be a valid numeric string
    ///   that can be parsed back to a `u32`.
    /// - For Gmail API accounts, this is the message ID (`mid`) returned by the API.
    pub id: String,
    /// A preview text for the reply email.
    ///
    /// This optional field provides a short summary or preview of the reply content.
    #[oai(validator(min_length = "1", max_length = "200"))]
    pub preview: Option<String>,
    /// The plain text body of the reply email.
    ///
    /// This field is optional and can be used to provide plain text content.
    #[oai(validator(min_length = "1", max_length = "10000"))]
    pub text: Option<String>,
    /// The HTML body of the reply email.
    ///
    /// This field is optional and can be used to provide HTML content.
    #[oai(validator(min_length = "1", max_length = "50000"))]
    pub html: Option<String>,
    /// The path of the folder used to store drafts (IMAP accounts only).
    ///
    /// For example: "[Gmail]/Drafts".
    /// This can be obtained from the `name` field of the mailbox via the list-mailboxes endpoint.
    /// For Gmail API accounts, this field is ignored.
    pub draft_folder_path: Option<String>,
}

impl AppendReplyToDraftRequest {
    fn validate(&self, is_gmail_api: bool) -> RustMailerResult<()> {
        if self.mailbox_name.trim().is_empty() {
            return Err(raise_error!(
                "mailbox_name cannot be empty".into(),
                ErrorCode::InvalidParameter
            ));
        }

        if !is_gmail_api {
            // IMAP account: uid and draft_folder_path required
            if self.id.parse::<u32>().is_err() {
                return Err(raise_error!(
                    "Invalid IMAP UID: `id` must be a numeric string".into(),
                    ErrorCode::InvalidParameter
                ));
            }
            if self
                .draft_folder_path
                .as_ref()
                .map(|s| s.is_empty())
                .unwrap_or(true)
            {
                return Err(raise_error!(
                    "draft_folder_path cannot be empty for IMAP accounts".into(),
                    ErrorCode::InvalidParameter
                ));
            }
        }
        Ok(())
    }

    pub async fn append_reply_to_draft(&self, account_id: u64) -> RustMailerResult<()> {
        let account = AccountModel::check_account_active(account_id, false).await?;
        self.validate(matches!(account.mailer_type, MailerType::GmailApi))?;

        match account.mailer_type {
            MailerType::ImapSmtp => self.append_reply_to_draft_imap(&account).await?,
            MailerType::GmailApi => {
                self.append_reply_to_draft_gmail(&account, account_id)
                    .await?
            }
            MailerType::GraphApi => todo!(),
        }

        Ok(())
    }

    async fn append_reply_to_draft_imap(&self, account: &AccountModel) -> RustMailerResult<()> {
        let envelope = EmailHandler::get_envelope(
            account,
            &self.mailbox_name,
            self.id.parse::<u32>().ok().unwrap(),
        )
        .await?;

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
            .subject(subject);
        builder = apply_references(builder, &envelope)?;
        builder = self.apply_content(builder)?;
        let message = builder.into_message().map_err(|e| {
            raise_error!(
                format!("Failed to build message: {}", e),
                ErrorCode::InternalError
            )
        })?;

        let executor = RUST_MAIL_CONTEXT.imap(account.id).await?;
        let drafts =
            encode_mailbox_name!(&self.draft_folder_path.as_ref().ok_or_else(|| raise_error!(
                "draft_folder_path is missing but required for IMAP accounts".into(),
                ErrorCode::InternalError
            ))?);
        executor.append(drafts, None, None, message.body).await?;
        Ok(())
    }

    async fn append_reply_to_draft_gmail(
        &self,
        account: &AccountModel,
        account_id: u64,
    ) -> RustMailerResult<()> {
        let target_label = GmailLabels::get_by_name(account_id, &self.mailbox_name).await?;
        let envelope = GmailEnvelope::find(account_id, target_label.id, &self.id)
            .await?
            .ok_or_else(|| {
                raise_error!(
                    format!(
                        "Gmail message with id '{}' not found in label '{}' for account {}",
                        self.id, target_label.name, account_id
                    ),
                    ErrorCode::ResourceNotFound
                )
            })?;

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
            .subject(subject);
        builder = apply_references2(builder, &envelope)?;
        builder = self.apply_content(builder)?;
        let message = builder.into_message().map_err(|e| {
            raise_error!(
                format!("Failed to build message: {}", e),
                ErrorCode::InternalError
            )
        })?;

        let raw_encoded = base64_encode_url_safe!(&message.body);
        let body = json!({
            "message": {
                "threadId": envelope.gmail_thread_id,
                "raw": raw_encoded
            }
        });

        GmailClient::create_draft(account_id, account.use_proxy, body).await?;
        Ok(())
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
