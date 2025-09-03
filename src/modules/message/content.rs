// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::account::v2::AccountV2;
use crate::modules::error::code::ErrorCode;
use crate::modules::imap::section::Encoding;
use crate::modules::message::attachment::inline_attachment_diskcache_key;
use crate::{
    encode_mailbox_name,
    modules::{
        cache::disk::DISK_CACHE,
        context::executors::RUST_MAIL_CONTEXT,
        error::RustMailerResult,
        imap::section::{EmailBodyPart, ImapAttachment, PartType, SegmentPath},
    },
    raise_error,
};

use cacache::Reader;
use mime_guess::from_ext;
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

const MAX_BODY_SIZE: usize = 2 * 1024 * 1024;

/// Request for fetching the content of a specific email message  
///  
/// This struct is used with the `fetch_message_content` method to retrieve  
/// the detailed content of an email message. The message identifier (mailbox and uid)  
/// typically comes from the results returned by the `list_messages` method.  
///  
/// When you call `list_messages`, it returns a paginated list of `EmailEnvelope` objects,  
/// each containing metadata about an email message. To fetch the full content of a specific  
/// message, you use the mailbox name and uid from that envelope in this request.  
///  
/// @param mailbox - The name of the mailbox containing the message (from list_messages results)  
/// @param uid - The unique identifier of the message within the mailbox (from list_messages results)  
/// @param max_length - Optional maximum length to retrieve for text parts (limits large messages)  
/// @param sections - Specific email body parts to retrieve (e.g., TEXT, HTML)  
/// @param inline - Optional list of attachments to include inline in the response  
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MessageContentRequest {
    /// The name of the mailbox containing the message  
    pub mailbox: String,

    /// The unique identifier of the message within the mailbox  
    pub uid: u32,

    /// Optional maximum length to retrieve for text parts (useful for large messages)  
    pub max_length: Option<usize>,

    /// Specific email body parts to retrieve (e.g., TEXT, HTML)  
    pub sections: Vec<EmailBodyPart>,

    /// Optional list of attachments to include inline in the response  
    pub inline: Option<Vec<ImapAttachment>>,
}

/// Represents the content of an email message in both plain text and HTML formats.
///
/// This struct contains optional fields for plain text and HTML versions of
/// the email message body. At least one of them may be present.
///
/// # Fields
///
/// - `plain`: The plain text version of the message, if available.
/// - `html`: The HTML version of the message, if available.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MessageContent {
    /// Optional plain text version of the message.
    pub plain: Option<PlainText>,

    /// Optional HTML version of the message.
    pub html: Option<String>,
}

impl MessageContent {
    pub fn html(&self) -> Option<&str> {
        self.html.as_deref()
    }

    pub fn plain(&self) -> Option<&str> {
        self.plain.as_ref().map(|plain| plain.content.as_str())
    }
}

/// Represents the plain text content of an email message.
///
/// This struct includes the actual plain text and a flag indicating whether
/// the content has been truncated.
///
/// # Fields
///
/// - `content`: The plain text body of the message.
/// - `truncated`: Indicates whether the content has been truncated,
///   for example, due to length limitations.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct PlainText {
    /// The plain text content of the message.
    pub content: String,

    /// Whether the content has been truncated.
    pub truncated: bool,
}

fn email_content_diskcache_key(
    account_id: u64,
    mailbox_name: &str,
    uid: u32,
    segment_path: SegmentPath,
) -> String {
    format!(
        "email_content_{}_{}_{}_{}",
        account_id, mailbox_name, uid, segment_path
    )
}

async fn read_text_from_reader(
    reader: &mut Reader,
    max_length: Option<usize>,
    actual_size: usize,
) -> RustMailerResult<PlainText> {
    let length_to_read = match max_length {
        Some(max) => max.min(actual_size).min(MAX_BODY_SIZE),
        None => actual_size,
    };
    let mut buffer = vec![0u8; length_to_read];
    let bytes_read = reader
        .read(&mut buffer)
        .await
        .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
    let truncated = bytes_read < actual_size;

    let content = match std::str::from_utf8(&buffer[..bytes_read]) {
        Ok(valid_str) => valid_str.into(),
        Err(_) => "???".into(),
    };

    Ok(PlainText { content, truncated })
}

async fn read_html_from_reader(
    reader: &mut Reader,
    actual_size: usize,
) -> RustMailerResult<String> {
    let mut buffer = vec![0u8; actual_size];
    let bytes_read = reader
        .read(&mut buffer)
        .await
        .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
    let content = match std::str::from_utf8(&buffer[..bytes_read]) {
        Ok(valid_str) => valid_str.into(),
        Err(_) => "???".into(),
    };
    Ok(content)
}

fn to_string(data: &[u8]) -> RustMailerResult<String> {
    let content = match std::str::from_utf8(data) {
        Ok(valid_str) => valid_str.into(),
        Err(_) => "???".into(),
    };
    Ok(content)
}

async fn replace_inline_attachments(
    account_id: u64,
    mailbox: &str,
    uid: u32,
    html_content: &mut String,
    inline_attachments: &[ImapAttachment],
    skip_cache: bool,
) -> RustMailerResult<()> {
    for attachment in inline_attachments {
        // only process inline attachments with content_id and base64 encoding, skip others
        if !attachment.inline
            || attachment.content_id.is_none()
            || attachment.transfer_encoding != Encoding::Base64
        {
            continue;
        }

        let cid = attachment
            .content_id
            .as_deref()
            .unwrap()
            .trim_matches(|c| c == '<' || c == '>');

        if !html_content.contains(cid) {
            continue;
        }

        let attachment_content = if skip_cache {
            // Skip cache and fetch directly from server
            let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
            let result = executor
                .uid_fetch_single_part(
                    uid.to_string().as_str(),
                    encode_mailbox_name!(mailbox).as_str(),
                    attachment.path.to_string().as_str(),
                )
                .await?;

            let target = result.iter().find(|f| f.uid == Some(uid)).ok_or_else(|| {
                raise_error!(
                    "Failed to fetch attachment from server".into(),
                    ErrorCode::InternalError
                )
            })?;

            let encoded = attachment.encoded(target).ok_or_else(|| {
                raise_error!(
                    "Failed to parse inline attachment content from result".into(),
                    ErrorCode::InternalError
                )
            })?;

            String::from_utf8_lossy(&encoded).into_owned()
        } else {
            // Try cache first, then fall back to server
            let inline_cache_key =
                inline_attachment_diskcache_key(account_id, mailbox, uid, attachment.path.clone());

            match DISK_CACHE.get_cache(&inline_cache_key).await? {
                Some(mut reader) => {
                    let mut str = String::new();
                    reader
                        .read_to_string(&mut str)
                        .await
                        .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
                    reader
                        .check()
                        .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
                    str
                }
                None => {
                    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
                    let result = executor
                        .uid_fetch_single_part(
                            uid.to_string().as_str(),
                            encode_mailbox_name!(mailbox).as_str(),
                            attachment.path.to_string().as_str(),
                        )
                        .await?;

                    let target = result.iter().find(|f| f.uid == Some(uid)).ok_or_else(|| {
                        raise_error!(
                            "Failed to fetch attachment from server".into(),
                            ErrorCode::InternalError
                        )
                    })?;

                    let encoded = attachment.encoded(target).ok_or_else(|| {
                        raise_error!(
                            "Failed to parse inline attachment content from result".into(),
                            ErrorCode::InternalError
                        )
                    })?;

                    DISK_CACHE
                        .put_cache(&inline_cache_key, &encoded, false)
                        .await?;
                    String::from_utf8_lossy(&encoded).into_owned()
                }
            }
        };

        let content_type = &attachment.file_type.to_ascii_lowercase();
        let mime_type = from_ext(content_type).first_or_octet_stream();
        let cleaned_content = attachment_content.replace("\n", "").replace("\r", "");
        *html_content = html_content.replace(
            &format!("cid:{}", cid),
            &format!("data:{};base64,{}", mime_type, cleaned_content.trim()),
        );
    }
    Ok(())
}

pub async fn retrieve_email_content(
    account_id: u64,
    request: MessageContentRequest,
    skip_cache: bool,
) -> RustMailerResult<MessageContent> {
    AccountV2::check_account_active(account_id, false).await?;

    let mut plain: Option<PlainText> = None;
    let mut html: Option<String> = None;

    // Find Plain part
    if let Some(part) = request
        .sections
        .iter()
        .find(|p| p.part_type == PartType::Plain)
    {
        let content = if skip_cache {
            // Skip cache and fetch directly
            let decoded_content =
                fetch_mail_part_from_imap(account_id, request.uid, &request.mailbox, part).await?;
            let mut decoded_content = to_string(&decoded_content)?;

            // Handle max_length truncation
            if matches!(request.max_length, Some(max) if decoded_content.len() > max) {
                decoded_content.truncate(request.max_length.unwrap());
                PlainText {
                    content: decoded_content,
                    truncated: true,
                }
            } else {
                PlainText {
                    content: decoded_content,
                    truncated: false,
                }
            }
        } else {
            // Try cache first
            let cache_key = email_content_diskcache_key(
                account_id,
                &request.mailbox,
                request.uid,
                part.path.clone(),
            );

            if let Some(mut reader) = DISK_CACHE.get_cache(&cache_key).await? {
                read_text_from_reader(&mut reader, request.max_length, part.size).await?
            } else {
                // Fetch from IMAP if not in cache
                let decoded_content =
                    fetch_mail_part_from_imap(account_id, request.uid, &request.mailbox, part)
                        .await?;
                // Cache the decoded content
                DISK_CACHE
                    .put_cache(&cache_key, decoded_content.as_slice(), false)
                    .await?;

                let mut decoded_content = to_string(&decoded_content)?;

                // Handle max_length truncation
                if matches!(request.max_length, Some(max) if decoded_content.len() > max) {
                    decoded_content.truncate(request.max_length.unwrap());
                    PlainText {
                        content: decoded_content,
                        truncated: true,
                    }
                } else {
                    PlainText {
                        content: decoded_content,
                        truncated: false,
                    }
                }
            }
        };
        plain = Some(content);
    }

    // Find HTML part
    if let Some(part) = request
        .sections
        .iter()
        .find(|p| p.part_type == PartType::Html)
    {
        let content = if skip_cache {
            // Skip cache and fetch directly
            let decoded_content =
                fetch_mail_part_from_imap(account_id, request.uid, &request.mailbox, part).await?;
            let mut decoded_content = to_string(&decoded_content)?;

            // Handle inline attachments
            if let Some(inline) = &request.inline {
                replace_inline_attachments(
                    account_id,
                    &request.mailbox,
                    request.uid,
                    &mut decoded_content,
                    inline,
                    skip_cache,
                )
                .await?;
            }
            decoded_content
        } else {
            // Try cache first
            let cache_key = email_content_diskcache_key(
                account_id,
                &request.mailbox,
                request.uid,
                part.path.clone(),
            );

            if let Some(mut reader) = DISK_CACHE.get_cache(&cache_key).await? {
                let mut content = read_html_from_reader(&mut reader, part.size).await?;
                if let Some(inline) = &request.inline {
                    replace_inline_attachments(
                        account_id,
                        &request.mailbox,
                        request.uid,
                        &mut content,
                        inline,
                        skip_cache,
                    )
                    .await?;
                }
                content
            } else {
                // Fetch from IMAP if not in cache
                let decoded_content =
                    fetch_mail_part_from_imap(account_id, request.uid, &request.mailbox, part)
                        .await?;
                // Cache the decoded content
                DISK_CACHE
                    .put_cache(&cache_key, decoded_content.as_slice(), false)
                    .await?;

                let mut decoded_content = to_string(&decoded_content)?;

                // Handle inline attachments
                if let Some(inline) = &request.inline {
                    replace_inline_attachments(
                        account_id,
                        &request.mailbox,
                        request.uid,
                        &mut decoded_content,
                        inline,
                        skip_cache,
                    )
                    .await?;
                }
                decoded_content
            }
        };
        html = Some(content);
    }

    Ok(MessageContent { plain, html })
}

async fn fetch_mail_part_from_imap(
    account_id: u64,
    uid: u32,
    mailbox: &str,
    part: &EmailBodyPart,
) -> RustMailerResult<Vec<u8>> {
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;

    let result = executor
        .uid_fetch_single_part(
            uid.to_string().as_str(),
            encode_mailbox_name!(mailbox).as_str(),
            part.path.to_string().as_str(),
        )
        .await?;

    let target = result.iter().find(|f| f.uid == Some(uid)).ok_or_else(|| {
        raise_error!(
            "Failed to fetch the specified content from the IMAP server".into(),
            ErrorCode::InternalError
        )
    })?;

    part.decode(target).ok_or_else(|| {
        raise_error!(
            "Failed to decode the email body content from the result".into(),
            ErrorCode::InternalError
        )
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_all_mime_types() {
        assert_eq!(from_ext("png").first_or_octet_stream(), "image/png");
        assert_eq!(from_ext("jpeg").first_or_octet_stream(), "image/jpeg");
        assert_eq!(from_ext("jpg").first_or_octet_stream(), "image/jpeg");
        assert_eq!(from_ext("json").first_or_octet_stream(), "application/json");
        assert_eq!(from_ext("txt").first_or_octet_stream(), "text/plain");
        assert_eq!(
            from_ext("unknown").first_or_octet_stream(),
            "application/octet-stream"
        );
        assert_eq!(from_ext("PNG").first_or_octet_stream(), "image/png");
        assert_eq!(from_ext("JpG").first_or_octet_stream(), "image/jpeg");
        assert_eq!(from_ext("mp4").first_or_octet_stream(), "video/mp4");
        assert_eq!(from_ext("webm").first_or_octet_stream(), "video/webm");
        assert_eq!(from_ext("avi").first_or_octet_stream(), "video/x-msvideo");
    }
}
