// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    modules::{
        account::migration::AccountModel,
        error::{code::ErrorCode, RustMailerResult},
        settings::cli::SETTINGS,
        smtp::{
            request::{
                builder::EmailBuilder,
                headers::HeaderValue,
                parser::{AttachmentFromEml, EmlData},
                EmailAddress, EmailHandler, MailAttachment, SendControl,
            },
            template::{entity::EmailTemplate, render::Templates},
            track::EmailTracker,
            util::generate_message_id,
        },
    },
    raise_error, utc_now, validate_email,
};

use mail_send::mail_builder::{headers::address::Address, MessageBuilder};
use mime_guess::Mime;
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

use std::{borrow::Cow, collections::HashMap};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct SendEmailRequest {
    /// The sender's email address.
    ///
    /// If not provided, a default sender address may be used based on the account configuration.
    pub from: Option<EmailAddress>,
    /// The list of recipients for the email.
    ///
    /// This field is required and must contain at least one recipient.
    /// Each recipient in the list will be handled as a separate send task (e.g., To, Cc, or Bcc).
    pub recipients: Vec<Recipient>,
    /// The subject line of the email.
    ///
    /// This field is optional and may be omitted for emails without a subject.
    pub subject: Option<String>,
    /// The plain text body of the email.
    ///
    /// This field is optional and can be used for emails that support plain text content.
    pub text: Option<String>,
    /// The HTML body of the email.
    ///
    /// This field is optional and can be used for emails that support HTML content.
    pub html: Option<String>,
    /// A preview text for the email.
    ///
    /// This optional field provides a short summary or preview of the email content, often displayed in email clients.
    pub preview: Option<String>,
    /// The raw EML content of the email.
    ///
    /// This optional field allows specifying the email content in EML format, overriding other content fields if provided.
    pub eml: Option<String>,
    /// The name of a template to use for rendering the email.
    ///
    /// This optional field specifies a predefined email template to generate the email content.
    pub template_id: Option<u64>,
    /// A list of attachments to include in the email.
    ///
    /// This optional field allows adding file attachments to the email.
    pub attachments: Option<Vec<MailAttachment>>,
    /// Custom email headers to include in the email.
    ///
    /// This optional field allows specifying additional headers (e.g., Reply-To, X-Custom-Header) as key-value pairs.
    pub headers: Option<HashMap<String, HeaderValue>>,
    /// Configuration options for controlling the email sending process.
    ///
    /// This required field specifies settings such as scheduling, or retry policies for sending the email.
    pub send_control: Option<SendControl>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct Recipient {
    /// The primary recipients of the email (To field).
    ///
    /// This field is required and must contain at least one email address to which the email is sent.
    pub to: Vec<EmailAddress>,
    /// The carbon copy (Cc) recipients of the email.
    ///
    /// This optional field specifies additional recipients who receive a copy of the email.
    pub cc: Option<Vec<EmailAddress>>,
    /// The blind carbon copy (Bcc) recipients of the email.
    ///
    /// This optional field specifies recipients who receive a copy of the email without being visible
    /// to other recipients.
    pub bcc: Option<Vec<EmailAddress>>,
    /// The reply-to email addresses for the email.
    ///
    /// This optional field specifies addresses to which replies should be sent, overriding the sender's address.
    pub reply_to: Option<Vec<EmailAddress>>,
    // Template parameters for rendering the email content.
    ///
    /// This optional field provides dynamic data (in JSON format) for use with email templates specified
    /// in the `SendEmailRequest`.
    pub template_params: Option<serde_json::Value>,
    /// The scheduled time to send the email, in milliseconds since the Unix epoch.
    ///
    /// This optional field allows specifying a future time for sending the email. If not provided,
    /// the email is sent immediately.
    pub send_at: Option<i64>,
}

impl Recipient {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.to.is_empty() {
            errors.push("Missing recipient: no valid 'to' address provided".into());
        }

        for email in &self.to {
            if validate_email!(&email.address).is_err() {
                errors.push(format!("Invalid 'to' email address: {}", &email.address));
            }
        }
        if let Some(cc) = &self.cc {
            for email in cc {
                if validate_email!(&email.address).is_err() {
                    errors.push(format!("Invalid 'cc' email address: {}", &email.address));
                }
            }
        }
        if let Some(bcc) = &self.bcc {
            for email in bcc {
                if validate_email!(&email.address).is_err() {
                    errors.push(format!("Invalid 'bcc' email address: {}", &email.address));
                }
            }
        }
        if let Some(reply_to) = &self.reply_to {
            for email in reply_to {
                if validate_email!(&email.address).is_err() {
                    errors.push(format!(
                        "Invalid 'reply_to' email address: {}",
                        &email.address
                    ));
                }
            }
        }

        if let Some(send_at) = self.send_at {
            if let Err(error) = EmailHandler::validate_send_at(send_at, utc_now!()) {
                errors.push(error);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

impl EmailBuilder for SendEmailRequest {
    async fn validate(&self) -> RustMailerResult<()> {
        let mut errors = Vec::new();

        if let Some(from) = &self.from {
            if validate_email!(&from.address).is_err() {
                errors.push("Invalid 'from' email address".into());
            }
        }

        if self.recipients.is_empty() {
            errors.push("At least one recipient is required".into());
        }

        for recipient in &self.recipients {
            if let Err(mut recipient_errors) = recipient.validate() {
                errors.append(&mut recipient_errors);
            }
        }

        if let Some(send_control) = &self.send_control {
            if let Err(mut send_control_error) = send_control.validate() {
                errors.append(&mut send_control_error);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(raise_error!(
                format!("{:#?}", errors),
                ErrorCode::InvalidParameter
            ))
        }
    }

    async fn build(&self, account_id: u64) -> RustMailerResult<()> {
        self.validate().await?;
        let account = &AccountModel::get(account_id).await?;
        let from = self.from.clone().map(Into::into).unwrap_or_else(|| {
            Address::new_address(
                account.name.as_ref().map(|n| Cow::Owned(n.to_string())),
                Cow::Owned(account.email.clone()),
            )
        });

        for recipient in &self.recipients {
            let mut builder = MessageBuilder::new().from(from.clone());
            let message_id = generate_message_id();
            builder = Self::apply_recipient_headers(builder, recipient, &message_id)?;
            if let Some(headers) = &self.headers {
                builder = headers.iter().fold(builder, |b, (k, v)| {
                    b.header(k.clone(), v.clone().to_header_type())
                });
            }
            let mut tracker: Option<EmailTracker> = None;

            if let Some(send_control) = &self.send_control {
                if let Some(true) = send_control.enable_tracking {
                    if SETTINGS.rustmailer_email_tracking_enabled {
                        let campaign_id = send_control
                            .campaign_id
                            .clone()
                            .unwrap_or_else(|| "default".to_string());

                        let recipient_address = recipient
                            .to
                            .first()
                            .map(|r| r.address.clone())
                            .unwrap_or_default();

                        tracker = Some(EmailTracker::new(
                            campaign_id,
                            message_id.clone(),
                            recipient_address,
                            account_id.into(),
                            account.email.clone(),
                        ));
                    }
                }
            }

            builder = match &self.eml {
                Some(eml) => Self::build_from_eml(builder, eml, tracker)?,
                None => {
                    self.build_content(builder, recipient, account, tracker)
                        .await?
                }
            };

            if let Some(send_control) = &self.send_control {
                let send_at = recipient.send_at.or(send_control.send_at);
                if let Some(send_at) = send_at {
                    builder = builder.date(send_at / 1000)
                }
            }

            EmailHandler::schedule_task(
                account,
                self.subject.clone(),
                message_id,
                recipient.cc.clone(),
                recipient.bcc.clone(),
                self.attachments.as_ref().map_or(0, |v| v.len()),
                builder,
                self.send_control.clone(),
                recipient
                    .send_at
                    .or_else(|| self.send_control.as_ref().and_then(|c| c.send_at)),
                None,
            )
            .await?;
        }

        Ok(())
    }
}

impl SendEmailRequest {
    fn apply_recipient_headers(
        mut builder: MessageBuilder<'static>,
        recipient: &Recipient,
        message_id: &str,
    ) -> RustMailerResult<MessageBuilder<'static>> {
        builder = builder.to(EmailHandler::to_address(&recipient.to)?);
        if let Some(cc) = &recipient.cc {
            builder = builder.cc(EmailHandler::to_address(cc)?);
        }
        if let Some(bcc) = &recipient.bcc {
            builder = builder.bcc(EmailHandler::to_address(bcc)?);
        }
        if let Some(reply_to) = &recipient.reply_to {
            builder = builder.reply_to(EmailHandler::to_address(reply_to)?);
        }
        builder = builder.message_id(message_id.to_string());
        Ok(builder)
    }

    fn build_from_eml(
        mut builder: MessageBuilder<'static>,
        eml: &str,
        tracker: Option<EmailTracker>,
    ) -> RustMailerResult<MessageBuilder<'static>> {
        let eml_data = EmlData::parse(eml)?;

        if let Some(subject) = eml_data.subject {
            builder = builder.subject(subject);
        }
        if let Some(text) = eml_data.text {
            builder = builder.text_body(text);
        }
        if let Some(html) = eml_data.html {
            match tracker {
                Some(mut tracker) => {
                    tracker.set_html(html);
                    tracker.track_links();
                    tracker.append_tracking_pixel()?;
                    let html = tracker.get_html().to_string();
                    builder = builder.html_body(html);
                }
                None => {
                    builder = builder.html_body(html);
                }
            }
        }

        if let Some(attachments) = eml_data.attachments {
            builder = Self::apply_eml_attachments(builder, &attachments)?;
        }

        Ok(builder)
    }

    async fn build_content(
        &self,
        mut builder: MessageBuilder<'static>,
        recipient: &Recipient,
        account: &AccountModel,
        tracker: Option<EmailTracker>,
    ) -> RustMailerResult<MessageBuilder<'static>> {
        if let Some(attachments) = &self.attachments {
            builder = Self::apply_mail_attachments(builder, attachments, account).await?;
        }

        match self.template_id {
            Some(id) => {
                let template = EmailTemplate::get(id).await?;
                let (subject, text, html) =
                    Templates::render(&template, &recipient.template_params)?;

                builder = builder.subject(subject);
                if let Some(text) = text {
                    builder = builder.text_body(text);
                }
                if let Some(html) = html {
                    match tracker {
                        Some(mut tracker) => {
                            tracker.set_html(html);
                            tracker.track_links();
                            tracker.append_tracking_pixel()?;
                            let html = tracker.get_html().to_string();
                            builder = builder.html_body(html);
                        }
                        None => {
                            builder = builder.html_body(html);
                        }
                    }
                }
            }
            None => {
                if let Some(subject) = &self.subject {
                    builder = builder.subject(subject.clone());
                }
                if let Some(text) = &self.text {
                    builder = builder.text_body(text.clone());
                }
                if let Some(html) = &self.html {
                    let html = EmailHandler::insert_preview(&self.preview, html.clone());
                    match tracker {
                        Some(mut tracker) => {
                            tracker.set_html(html);
                            tracker.track_links();
                            tracker.append_tracking_pixel()?;
                            let html = tracker.get_html().to_string();
                            builder = builder.html_body(html);
                        }
                        None => {
                            builder = builder.html_body(html);
                        }
                    }
                }
            }
        }

        Ok(builder)
    }

    fn apply_eml_attachments(
        mut builder: MessageBuilder<'static>,
        attachments: &[AttachmentFromEml],
    ) -> RustMailerResult<MessageBuilder<'static>> {
        for attachment in attachments {
            let mime = attachment.mime_type.parse::<Mime>().map_err(|e| {
                raise_error!(
                    format!("Invalid MIME type: {}", e),
                    ErrorCode::EmlFileParseError
                )
            })?;

            builder = if attachment.inline {
                builder.inline(
                    mime.to_string(),
                    attachment.content_id.clone().ok_or_else(|| {
                        raise_error!(
                            "Missing content_id for inline attachment".into(),
                            ErrorCode::EmlFileParseError
                        )
                    })?,
                    attachment.content.clone(),
                )
            } else {
                builder.attachment(
                    mime.to_string(),
                    attachment.file_name.clone().ok_or_else(|| {
                        raise_error!(
                            "Missing file_name for attachment".into(),
                            ErrorCode::EmlFileParseError
                        )
                    })?,
                    attachment.content.clone(),
                )
            };
        }
        Ok(builder)
    }

    async fn apply_mail_attachments(
        mut builder: MessageBuilder<'static>,
        attachments: &[MailAttachment],
        account: &AccountModel,
    ) -> RustMailerResult<MessageBuilder<'static>> {
        for attachment in attachments {
            let content = attachment.get_content(account).await?;
            let mime = attachment.mime_type.parse::<Mime>().map_err(|e| {
                raise_error!(
                    format!("Invalid MIME type: {}", e),
                    ErrorCode::InvalidParameter
                )
            })?;

            builder = if attachment.inline {
                builder.inline(
                    mime.to_string(),
                    attachment.content_id.clone().ok_or_else(|| {
                        raise_error!(
                            "Missing content_id for inline attachment".into(),
                            ErrorCode::InvalidParameter
                        )
                    })?,
                    content,
                )
            } else {
                builder.attachment(
                    mime.to_string(),
                    attachment.file_name.clone().ok_or_else(|| {
                        raise_error!(
                            "Missing file_name for attachment".into(),
                            ErrorCode::InvalidParameter
                        )
                    })?,
                    content,
                )
            };
        }
        Ok(builder)
    }
}
