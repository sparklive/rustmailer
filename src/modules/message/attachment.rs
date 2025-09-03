// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::error::code::ErrorCode;
use crate::modules::message::get_minimal_meta;
use crate::{
    encode_mailbox_name,
    modules::account::v2::AccountV2,
    modules::cache::disk::DISK_CACHE,
    modules::context::executors::RUST_MAIL_CONTEXT,
    modules::error::RustMailerResult,
    modules::imap::section::{ImapAttachment, SegmentPath},
    raise_error,
};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

const MAX_ATTACHMENT_SIZE: usize = 52_428_800; // 50MB

/// Represents a request to fetch an attachment from a message in a mailbox.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct AttachmentRequest {
    /// The UID of the message containing the attachment.
    pub uid: u32,

    /// The name of the mailbox where the message is located.
    pub mailbox: String,

    /// The metadata describing the attachment to fetch.
    pub attachment: ImapAttachment,
}

pub fn attachment_diskcache_key(
    account_id: u64,
    mailbox_name: &str,
    uid: u32,
    segment_path: SegmentPath,
) -> String {
    format!(
        "email_attachment_{}_{}_{}_{}",
        account_id, mailbox_name, uid, segment_path
    )
}

pub fn inline_attachment_diskcache_key(
    account_id: u64,
    mailbox_name: &str,
    uid: u32,
    segment_path: SegmentPath,
) -> String {
    format!(
        "email_inline_attachment_{}_{}_{}_{}",
        account_id, mailbox_name, uid, segment_path
    )
}

pub async fn retrieve_email_attachment(
    account_id: u64,
    request: AttachmentRequest,
) -> RustMailerResult<cacache::Reader> {
    AccountV2::check_account_active(account_id, false).await?;

    if request.attachment.size >= MAX_ATTACHMENT_SIZE {
        return Err(raise_error!(
            format!(
                "Attachment size {} bytes exceeds the maximum allowed size of {} bytes",
                request.attachment.size, MAX_ATTACHMENT_SIZE
            ),
            ErrorCode::ExceedsLimitation
        ));
    }

    let cache_key = attachment_diskcache_key(
        account_id,
        &request.mailbox,
        request.uid,
        request.attachment.path.clone(),
    );
    if let Some(reader) = DISK_CACHE.get_cache(&cache_key).await? {
        return Ok(reader);
    }

    let meta = get_minimal_meta(account_id, &request.mailbox, request.uid).await?;
    let attachments = meta.attachments.ok_or_else(|| {
        raise_error!(
            "No attachments found in the message".into(),
            ErrorCode::ResourceNotFound
        )
    })?;

    let target = attachments
        .into_iter()
        .find(|a| a.path == request.attachment.path)
        .ok_or_else(|| {
            raise_error!(
                format!(
                    "Attachment not found with path: {}",
                    request.attachment.path
                ),
                ErrorCode::ResourceNotFound
            )
        })?;

    if target.size >= MAX_ATTACHMENT_SIZE {
        return Err(raise_error!(format!(
            "Attachment size {} bytes exceeds the maximum allowed size of {} bytes. Attachment path: {}",
            target.size,
            MAX_ATTACHMENT_SIZE,
            target.path
        ), ErrorCode::ExceedsLimitation));
    }

    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    // Fetch the attachment from the server
    let result = executor
        .uid_fetch_single_part(
            &request.uid.to_string(),
            &encode_mailbox_name!(&request.mailbox),
            &request.attachment.path.to_string(),
        )
        .await?;
    // Find the corresponding result
    let target = result
        .iter()
        .find(|f| f.uid == Some(request.uid))
        .ok_or_else(|| {
            raise_error!(
                "Failed to fetch attachment from server".into(),
                ErrorCode::InternalError
            )
        })?;
    // Decode the attachment
    let decoded = request.attachment.decode(target).ok_or_else(|| {
        raise_error!(
            "Failed to parse attachment content from result".into(),
            ErrorCode::InternalError
        )
    })?;
    // Cache the result and return it
    DISK_CACHE.put_cache(&cache_key, &decoded, false).await?;

    // Cache the original inline attachment for replace cid with attachment content
    if request.attachment.inline {
        let encoded = request.attachment.encoded(target).ok_or_else(|| {
            raise_error!(
                "Failed to parse inline attachment content from result".into(),
                ErrorCode::InternalError
            )
        })?;
        let inline_cache_key = inline_attachment_diskcache_key(
            account_id,
            &request.mailbox,
            request.uid,
            request.attachment.path.clone(),
        );
        DISK_CACHE
            .put_cache(&inline_cache_key, &encoded, false)
            .await?;
    }

    DISK_CACHE
        .get_cache(&cache_key)
        .await?
        .ok_or_else(|| raise_error!("Unexpected cache miss".into(), ErrorCode::InternalError))
}
