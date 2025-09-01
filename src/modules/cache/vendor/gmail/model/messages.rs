// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::{
    modules::{
        cache::vendor::gmail::sync::envelope::GmailEnvelope,
        common::Addr,
        error::{code::ErrorCode, RustMailerError},
    },
    raise_error,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageIndex {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageList {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<MessageIndex>>,
    #[serde(rename = "nextPageToken")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
    #[serde(rename = "resultSizeEstimate")]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_size_estimate: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageMeta {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    #[serde(rename = "historyId")]
    pub history_id: String,
    #[serde(rename = "internalDate")]
    pub internal_date: String,
    #[serde(rename = "labelIds")]
    pub label_ids: Vec<String>,
    pub payload: Payload,
    #[serde(rename = "sizeEstimate")]
    pub size_estimate: u32,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Payload {
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub headers: Vec<Header>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Header {
    pub name: String,
    pub value: String,
}

impl TryFrom<MessageMeta> for GmailEnvelope {
    type Error = RustMailerError;

    fn try_from(value: MessageMeta) -> Result<Self, Self::Error> {
        let payload = value.payload;
        let mut envelope = Self {
            account_id: 0,
            label_id: 0,
            label_name: "".into(),
            id: value.id,
            internal_date: value.internal_date.parse().map_err(|e| {
                raise_error!(
                    format!("Failed to parse internal_date: {}", e),
                    ErrorCode::InternalError
                )
            })?,
            size: value.size_estimate,
            bcc: None,
            cc: None,
            date: None,
            from: None,
            in_reply_to: None,
            sender: None,
            message_id: None,
            subject: None,
            thread_id: 0,
            mime_version: None,
            references: None,
            reply_to: None,
            to: None,
            snippet: value.snippet,
            history_id: value.history_id,
            gmail_thread_id: value.thread_id,
            label_ids: value.label_ids,
        };

        for header in payload.headers {
            match header.name.as_str() {
                "Date" => {
                    let dt = DateTime::parse_from_rfc2822(&header.value).map_err(|e| {
                        raise_error!(
                            format!("Failed to parse Date: {}", e),
                            ErrorCode::InternalError
                        )
                    })?;
                    envelope.date = Some(dt.timestamp_millis());
                }
                "From" => envelope.from = Some(Addr::parse(&header.value)),
                "Sender" => envelope.sender = Some(Addr::parse(&header.value)),
                "Reply-To" => envelope.reply_to = Some(Self::parse_addr_list(&header.value)),
                "In-Reply-To" => {
                    envelope.in_reply_to = Some(Self::clean_angle_brackets(&header.value).into())
                }
                "Message-ID" => {
                    envelope.message_id = Some(Self::clean_angle_brackets(&header.value).into())
                }
                "Mime-Version" => envelope.mime_version = Some(header.value),
                "References" => {
                    envelope.references = Some(
                        header
                            .value
                            .split_whitespace()
                            .map(Self::clean_angle_brackets)
                            .filter(|id| !id.is_empty())
                            .map(|id| id.to_string())
                            .collect(),
                    )
                }
                "Subject" => envelope.subject = Some(header.value),
                "To" => envelope.to = Some(Self::parse_addr_list(&header.value)),
                "Bcc" => envelope.bcc = Some(Self::parse_addr_list(&header.value)),
                "Cc" => envelope.cc = Some(Self::parse_addr_list(&header.value)),
                _ => {}
            }
        }

        Ok(envelope)
    }
}
