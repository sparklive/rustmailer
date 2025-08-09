// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::time::Instant;

use native_db::*;
use native_model::{native_model, Model};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::{
    modules::{
        cache::imap::{envelope_v2::EmailEnvelopeV2, manager::EnvelopeFlagsManager},
        database::{
            batch_delete_impl, batch_insert_impl, delete_impl, filter_by_secondary_key_impl,
            manager::DB_MANAGER, update_impl,
        },
        error::{code::ErrorCode, RustMailerResult},
        utils::envelope_hash,
    },
    raise_error,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[native_model(id = 3, version = 1)]
#[native_db(primary_key(pk -> u64))]
pub struct MinimalEnvelope {
    /// The ID of the account owning the email.
    #[secondary_key]
    pub account_id: u64,
    /// The unique identifier of the mailbox where the email is stored (e.g., `MailBox::id`).
    /// Used for indexing to avoid updating indexes when mailboxes are renamed.
    #[secondary_key]
    pub mailbox_id: u64,
    /// The unique identifier (IMAP UID) of the email within the mailbox.
    pub uid: u32,
    /// A hash of the email's flags for efficient comparison or indexing.
    pub flags_hash: u64,
}

impl MinimalEnvelope {
    /// Generates a primary key to ensure ordered storage of email metadata by internal_date.
    pub fn pk(&self) -> u64 {
        envelope_hash(self.account_id, self.mailbox_id, self.uid)
    }

    pub async fn list_by_account(account_id: u64) -> RustMailerResult<Vec<MinimalEnvelope>> {
        filter_by_secondary_key_impl(
            DB_MANAGER.envelope_db(),
            MinimalEnvelopeKey::account_id,
            account_id,
        )
        .await
    }

    pub async fn batch_insert(envelopes: Vec<MinimalEnvelope>) -> RustMailerResult<()> {
        for e in &envelopes {
            EnvelopeFlagsManager::update_flag_change(
                e.account_id,
                e.mailbox_id,
                e.uid,
                e.flags_hash,
            );
        }
        batch_insert_impl(DB_MANAGER.envelope_db(), envelopes).await?;
        Ok(())
    }

    pub async fn clean_mailbox_envelopes(account_id: u64, mailbox_id: u64) -> RustMailerResult<()> {
        const BATCH_SIZE: usize = 500;
        let mut total_deleted = 0usize;
        let start_time = Instant::now();
        loop {
            let deleted = batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
                let to_delete: Vec<MinimalEnvelope> = rw
                    .scan()
                    .secondary(MinimalEnvelopeKey::mailbox_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .start_with(mailbox_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .take(BATCH_SIZE)
                    .filter_map(Result::ok) // filter only Ok values
                    .filter(|e: &MinimalEnvelope| e.account_id == account_id)
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
        mailbox_hash: u64,
        to_delete_uid: &[u32],
    ) -> RustMailerResult<()> {
        for uid in to_delete_uid {
            let key = envelope_hash(account_id, mailbox_hash, *uid);
            delete_impl(DB_MANAGER.envelope_db(), move |rw| {
                rw.get()
                    .primary::<MinimalEnvelope>(key)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .ok_or_else(|| {
                        raise_error!("minimal envelope missing".into(), ErrorCode::InternalError)
                    })
            })
            .await?;
        }
        Ok(())
    }

    pub async fn update_flags(
        account_id: u64,
        mailbox_id: u64,
        uid: u32,
        flags_hash: u64,
    ) -> RustMailerResult<()> {
        update_impl(
            DB_MANAGER.envelope_db(),
            move |rw| {
                rw.get()
                    .primary::<MinimalEnvelope>(envelope_hash(account_id, mailbox_id, uid))
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .ok_or_else(|| {
                        raise_error!(
                            "The MinimalEnvelope that you want to modify was not found."
                                .to_string(),
                            ErrorCode::ResourceNotFound
                        )
                    })
            },
            move |current| {
                let mut updated = current.clone();
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

    pub async fn clean_account(account_id: u64) -> RustMailerResult<()> {
        const BATCH_SIZE: usize = 500;
        let mut total_deleted = 0usize;
        let start_time = Instant::now();
        loop {
            let deleted = batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
                let to_delete: Vec<MinimalEnvelope> = rw
                    .scan()
                    .secondary(MinimalEnvelopeKey::account_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .start_with(account_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .take(BATCH_SIZE)
                    .filter_map(Result::ok) // filter only Ok values
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
            "Finished deleting envelopes for account_id={} total_deleted={} in {:?}",
            account_id,
            total_deleted,
            start_time.elapsed()
        );
        Ok(())
    }
}

impl From<&EmailEnvelopeV2> for MinimalEnvelope {
    fn from(value: &EmailEnvelopeV2) -> Self {
        Self {
            account_id: value.account_id,
            mailbox_id: value.mailbox_id,
            uid: value.uid,
            flags_hash: value.flags_hash,
        }
    }
}
