// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::cache::imap::address::AddressEntity;
use crate::modules::cache::imap::mailbox::EnvelopeFlag;
use crate::modules::cache::imap::manager::EnvelopeFlagsManager;
use crate::modules::cache::imap::minimal::MinimalEnvelope;
use crate::modules::common::Addr;
use crate::modules::database::manager::DB_MANAGER;
use crate::modules::database::{
    batch_delete_impl, delete_impl, paginate_secondary_scan_impl, secondary_find_impl, update_impl,
    with_transaction,
};
use crate::modules::error::code::ErrorCode;
use crate::modules::error::RustMailerResult;
use crate::modules::imap::section::{EmailBodyPart, ImapAttachment};
use crate::modules::rest::response::DataPage;
use crate::modules::utils::envelope_hash;
use crate::{raise_error, utc_now};
use itertools::Itertools;
use mail_parser::Received as OriginalReceived;
use native_db::*;
use native_model::{native_model, Model};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{error, info};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
#[native_model(id = 1, version = 1)]
#[native_db(primary_key(pk -> String), secondary_key(create_envelope_hash -> u64, unique))]
pub struct EmailEnvelope {
    /// The ID of the account owning the email.
    #[secondary_key]
    pub account_id: u64,
    /// The unique identifier of the mailbox where the email is stored (e.g., `MailBox::id`).
    /// Used for indexing to avoid updating indexes when mailboxes are renamed.
    #[secondary_key]
    pub mailbox_id: u64,
    /// The decoded, human-readable name of the mailbox (e.g., "INBOX", "Sent").
    pub mailbox_name: String,
    /// The unique identifier (IMAP UID) of the email within the mailbox.
    pub uid: u32,
    /// The date and time the email was received by the server, as a Unix timestamp in milliseconds.
    /// If `None`, the internal date is unavailable.
    pub internal_date: Option<i64>,
    /// The size of the email in bytes.
    pub size: u32,
    /// The flags associated with the email (e.g., `\Seen`, `\Answered`, `\Flagged`).
    /// Represented as a list of `EnvelopeFlag` for standard or custom flags.
    pub flags: Vec<EnvelopeFlag>,
    /// A hash of the email's flags for efficient comparison or indexing.
    pub flags_hash: u64,
    /// The blind carbon copy (BCC) recipient(s) of the email, if any.
    pub bcc: Option<Vec<Addr>>,
    /// The carbon copy (CC) recipient(s) of the email, if any.
    pub cc: Option<Vec<Addr>>,
    /// The date the email was sent, as a Unix timestamp in milliseconds, if available.
    pub date: Option<i64>,
    /// The sender's address, including name and email, if available.
    pub from: Option<Addr>,
    /// The message ID of the email to which this email is a reply, if applicable.
    pub in_reply_to: Option<String>,
    /// The actual sender's address, if different from the `from` field.
    pub sender: Option<Addr>,
    /// The return address for undeliverable emails, if specified.
    pub return_address: Option<String>,
    /// The unique message ID of the email, typically used for threading.
    pub message_id: Option<String>,
    /// The subject of the email, if available.
    pub subject: Option<String>,
    /// The name of the thread this email belongs to, if applicable.
    pub thread_name: Option<String>,
    /// The MIME version of the email (e.g., "1.0"), if specified.
    pub mime_version: Option<String>,
    /// A list of message IDs referenced by this email, used for threading.
    pub references: Option<Vec<String>>,
    /// The address(es) to which replies should be sent, if specified.
    pub reply_to: Option<Vec<Addr>>,
    /// The primary recipient(s) of the email, if any.
    pub to: Option<Vec<Addr>>,
    /// A list of attachments included in the email, if any.
    ///
    /// Each `ImapAttachment` item contains metadata including the part ID and MIME type,
    /// which indicates the exact location of the attachment in the raw message structure.
    /// This allows the backend to directly fetch specific attachments without retrieving
    /// the entire message content.
    ///
    /// This is particularly useful for accounts configured with minimal sync, where full
    /// message bodies are not cached locally. By including this data in the API response,
    /// the client can request to download only the required attachment via a follow-up
    /// API call, improving both efficiency and user experience.
    ///
    /// Developers do not need to understand the internal IMAP part structure — this
    /// metadata provides a clean abstraction for fetching specific attachments.
    pub attachments: Option<Vec<ImapAttachment>>,
    /// Metadata for the email's body parts (e.g., plain text, HTML), if available.
    ///
    /// Each `EmailBodyPart` contains detailed metadata (such as part ID, content type,
    /// and charset) describing a portion of the email body. This enables precise access
    /// to body content, such as plain text or HTML sections, without downloading the full
    /// raw message from the server.
    ///
    /// This is especially helpful for lightweight clients or minimized-sync accounts that
    /// do not cache full email content. The frontend can pass this metadata back to the
    /// server to retrieve only the desired portion of the message (e.g., the HTML body),
    /// which significantly reduces bandwidth and latency.
    ///
    /// By abstracting the complexity of MIME part navigation, developers can efficiently
    /// retrieve specific parts of an email without handling the low-level IMAP structure.
    pub body_meta: Option<Vec<EmailBodyPart>>,
    /// Details about how the email was received, if available.
    pub received: Option<Received>,
}

impl EmailEnvelope {
    pub fn pk(&self) -> String {
        format!(
            "{}_{}",
            self.internal_date.unwrap_or(utc_now!()),
            envelope_hash(self.account_id, self.mailbox_id, self.uid)
        )
    }

    pub fn create_envelope_hash(&self) -> u64 {
        envelope_hash(self.account_id, self.mailbox_id, self.uid)
    }

    pub async fn find(
        account_id: u64,
        mailbox_id: u64,
        uid: u32,
    ) -> RustMailerResult<Option<EmailEnvelope>> {
        secondary_find_impl(
            DB_MANAGER.envelope_db(),
            EmailEnvelopeKey::create_envelope_hash,
            envelope_hash(account_id, mailbox_id, uid),
        )
        .await
    }

    pub async fn get(envelope_hash: u64) -> RustMailerResult<Option<EmailEnvelope>> {
        secondary_find_impl(
            DB_MANAGER.envelope_db(),
            EmailEnvelopeKey::create_envelope_hash,
            envelope_hash,
        )
        .await
    }

    pub async fn save_envelopes(envelopes: Vec<EmailEnvelope>) -> RustMailerResult<()> {
        with_transaction(DB_MANAGER.envelope_db(), move |rw| {
            for e in envelopes {
                // Create a minimal representation of the envelope to speed up UID and flags_hash lookup
                // This avoids the need to deserialize the full EmailEnvelope during sync
                let minimal: MinimalEnvelope = MinimalEnvelope::from(&e);

                EnvelopeFlagsManager::update_flag_change(
                    e.account_id,
                    e.mailbox_id,
                    e.uid,
                    e.flags_hash,
                );

                // Extract address entities (e.g. from, to, cc) for indexing
                // This allows searching emails by address, especially useful since to/cc are vectors
                let address_entities = AddressEntity::extract(&e);

                // Store the full envelope
                rw.insert::<EmailEnvelope>(e)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;

                // Store the minimal envelope for quick access during sync
                rw.insert::<MinimalEnvelope>(minimal)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;

                // Store each address entity to enable address-based search
                for a in address_entities {
                    rw.insert::<AddressEntity>(a)
                        .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
                }
            }
            Ok(())
        })
        .await
    }

    pub async fn list_messages_in_mailbox(
        mailbox_id: u64,
        page: u64,
        page_size: u64,
        desc: bool,
    ) -> RustMailerResult<DataPage<EmailEnvelope>> {
        paginate_secondary_scan_impl(
            DB_MANAGER.envelope_db(),
            Some(page),
            Some(page_size),
            Some(desc),
            EmailEnvelopeKey::mailbox_id,
            mailbox_id,
        )
        .await
        .map(DataPage::from)
    }

    pub async fn update_flags(
        account_id: u64,
        mailbox_id: u64,
        uid: u32,
        flags: &[EnvelopeFlag],
        flags_hash: u64,
    ) -> RustMailerResult<()> {
        let flags = flags.to_vec();

        update_impl(
            DB_MANAGER.envelope_db(),
            move |rw| {
                rw.get()
                    .secondary::<EmailEnvelope>(
                        EmailEnvelopeKey::create_envelope_hash,
                        envelope_hash(account_id, mailbox_id, uid),
                    )
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .ok_or_else(|| {
                        raise_error!(
                            "The EmailEnvelope that you want to modify was not found.".to_string(),
                            ErrorCode::ResourceNotFound
                        )
                    })
            },
            move |current| {
                let mut updated = current.clone();
                updated.flags = flags;
                updated.flags_hash = flags_hash;
                Ok(updated)
            },
        )
        .await
        .map_err(|e| {
            error!(
                "Failed to update flags: account_id={}, mailbox_hash={}, uid={}, error={:?}",
                account_id, mailbox_id, uid, e
            );
            e
        })?;

        Ok(())
    }

    pub async fn clean_mailbox_envelopes(account_id: u64, mailbox_id: u64) -> RustMailerResult<()> {
        const BATCH_SIZE: usize = 200;
        let mut total_deleted = 0usize;
        let start_time = Instant::now();
        loop {
            let deleted = batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
                let to_delete: Vec<EmailEnvelope> = rw
                    .scan()
                    .secondary(EmailEnvelopeKey::mailbox_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .start_with(mailbox_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .take(BATCH_SIZE)
                    .filter_map(Result::ok) // filter only Ok values
                    .filter(|e: &EmailEnvelope| e.account_id == account_id)
                    .collect();
                Ok(to_delete)
            })
            .await?;
            total_deleted += deleted;
            // If this batch is empty, break the loop
            if deleted == 0 {
                break;
            }
        }

        info!(
            "Finished deleting envelopes for mailbox_hash={} account_id={} total_deleted={} in {:?}",
            mailbox_id,
            account_id,
            total_deleted,
            start_time.elapsed()
        );
        Ok(())
    }

    pub async fn clean_envelopes(
        account_id: u64,
        mailbox_id: u64,
        to_delete_uid: &[u32],
    ) -> RustMailerResult<()> {
        for uid in to_delete_uid {
            let key = envelope_hash(account_id, mailbox_id, *uid);
            delete_impl(DB_MANAGER.envelope_db(), move |rw| {
                rw.get()
                    .secondary::<EmailEnvelope>(EmailEnvelopeKey::create_envelope_hash, key)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .ok_or_else(|| {
                        raise_error!("envelope missing".into(), ErrorCode::InternalError)
                    })
            })
            .await?;
        }
        Ok(())
    }

    pub async fn clean_account(account_id: u64) -> RustMailerResult<()> {
        const BATCH_SIZE: usize = 200;
        let mut total_deleted = 0usize;
        let start_time = Instant::now();
        loop {
            let deleted = batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
                let to_delete: Vec<EmailEnvelope> = rw
                    .scan()
                    .secondary(EmailEnvelopeKey::account_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .start_with(account_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .take(BATCH_SIZE)
                    .try_collect()
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
                Ok(to_delete)
            })
            .await?;
            total_deleted += deleted;
            // If this batch is empty, break the loop
            if deleted == 0 {
                break;
            }
        }

        info!(
            "Finished deleting envelopes for account_id={} total_deleted={} in {:?}",
            account_id,
            total_deleted,
            start_time.elapsed()
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct Received {
    /// The server or host from which the email was received, if available.
    pub from: Option<String>,
    /// The server or host that received the email, if available.
    pub by: Option<String>,
    /// The protocol used to receive the email (e.g., "SMTP", "ESMTP"), if specified.
    pub with: Option<String>,
    /// The date and time the email was received, as a Unix timestamp in milliseconds, if available.
    pub date: Option<i64>,
}

impl<'x> From<&OriginalReceived<'x>> for Received {
    fn from(value: &OriginalReceived<'x>) -> Self {
        let convert_host = |host: &mail_parser::Host<'x>| match host {
            mail_parser::Host::Name(cow) => cow.to_string(),
            mail_parser::Host::IpAddr(ip_addr) => ip_addr.to_string(),
        };

        let from = value.from.as_ref().map(convert_host);
        let by = value.by.as_ref().map(convert_host);
        let with = value.with.as_ref().map(|p| p.to_string());
        let date = value.date.map(|d| d.to_timestamp() * 1000);

        Self {
            from,
            by,
            with,
            date,
        }
    }
}
