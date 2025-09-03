// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::{
    account::{since::DateSince, v2::AccountV2},
    cache::vendor::gmail::sync::{
        flow::{fetch_and_save_full_label, fetch_and_save_since_date, max_history_id},
        labels::{GmailCheckPoint, GmailLabels},
    },
    error::RustMailerResult,
};
use std::time::Instant;
use tracing::{error, info, warn};

pub async fn rebuild_cache(
    account: &AccountV2,
    remote_labels: &[GmailLabels],
) -> RustMailerResult<()> {
    let start_time = Instant::now();
    let mut total_inserted = 0;

    GmailLabels::batch_insert(remote_labels).await?;
    let mut history_ids = Vec::with_capacity(remote_labels.len());

    for label in remote_labels {
        if label.exists == 0 {
            info!(
                "Account {}: Label '{}' on the remote server has no emails. Skipping fetch for this label.",
                account.id, &label.name
            );
            continue;
        }
        match fetch_and_save_full_label(account, label, label.exists, true).await {
            Ok((inserted, max_history_id)) => {
                total_inserted += inserted;

                if let Some(history_id) = max_history_id {
                    history_ids.push(history_id);
                }
            }
            Err(e) => {
                warn!(
                    "Account {}: Failed to sync label '{}'. Error: {:#?}. Removing label entry.",
                    account.id, &label.name, e
                );
                if let Err(del_err) = GmailLabels::delete(label.id).await {
                    error!(
                        "Account {}: Failed to delete label '{}' after sync error: {}",
                        account.id, &label.name, del_err
                    );
                }
            }
        }
    }
    let max = max_history_id(&history_ids);
    if let Some(history_id) = max {
        let checkpoint = GmailCheckPoint::new(account.id, history_id.to_string());
        checkpoint.save().await?;
    }
    let elapsed_time = start_time.elapsed().as_secs();
    info!(
        "Rebuild account cache completed: {} envelopes inserted. {} secs elapsed. \
        This is a full data fetch as there was no local cache data available.",
        total_inserted, elapsed_time
    );
    Ok(())
}

pub async fn rebuild_cache_since_date(
    account: &AccountV2,
    remote_labels: &[GmailLabels],
    date_since: &DateSince,
) -> RustMailerResult<()> {
    let start_time = Instant::now();
    let mut total_inserted = 0;
    let date = date_since.since_gmail_date()?;

    GmailLabels::batch_insert(remote_labels).await?;
    let mut history_ids = Vec::with_capacity(remote_labels.len());
    for label in remote_labels {
        if label.exists == 0 {
            info!(
                "Account {}: Mailbox '{}' on the remote server has no emails. Skipping fetch for this mailbox.",
                account.id, &label.name
            );
            continue;
        }

        match fetch_and_save_since_date(account, date.as_str(), label, true).await {
            Ok((inserted, max_history_id)) => {
                total_inserted += inserted;
                if let Some(history_id) = max_history_id {
                    history_ids.push(history_id);
                }
            }
            Err(e) => {
                warn!(
                    "Account {}: Failed to sync mailbox '{}'. Error: {}. Removing mailbox entry.",
                    account.id, &label.name, e
                );
                if let Err(del_err) = GmailLabels::delete(label.id).await {
                    error!(
                        "Account {}: Failed to delete mailbox '{}' after sync error: {}",
                        account.id, &label.name, del_err
                    );
                }
            }
        }
    }

    let max = max_history_id(&history_ids);
    if let Some(history_id) = max {
        let checkpoint = GmailCheckPoint::new(account.id, history_id.to_string());
        checkpoint.save().await?;
    }
    let elapsed_time = start_time.elapsed().as_secs();
    info!(
        "Rebuild account cache completed: {} envelopes inserted. {} secs elapsed. \
        Data fetched from server starting from the specified date: {}.",
        total_inserted, elapsed_time, date
    );
    Ok(())
}

pub async fn rebuild_single_label_cache(
    account: &AccountV2,
    label: &GmailLabels,
) -> RustMailerResult<Option<String>> {
    if label.exists > 0 {
        match &account.date_since {
            Some(date_since) => {
                let date = date_since.since_gmail_date()?;
                match fetch_and_save_since_date(account, date.as_str(), label, true).await {
                    Ok((inserted, max_history_id)) => {
                        info!(
                            "Account {}: Label '{}' synced successfully. {} messages inserted.",
                            account.id, label.name, inserted
                        );
                        return Ok(max_history_id);
                    }
                    Err(e) => {
                        warn!(
                                    "Account {}: Failed to sync mailbox '{}'. Error: {}. Removing label entry.",
                                    account.id, &label.name, e
                                );
                        if let Err(del_err) = GmailLabels::delete(label.id).await {
                            error!(
                                "Account {}: Failed to delete mailbox '{}' after sync error: {}",
                                account.id, &label.name, del_err
                            );
                        }
                    }
                }
            }
            None => match fetch_and_save_full_label(account, label, label.exists, true).await {
                Ok((inserted, max_history_id)) => {
                    info!(
                        "Account {}: Label '{}' synced successfully. {} messages inserted.",
                        account.id, label.name, inserted
                    );
                    return Ok(max_history_id);
                }
                Err(e) => {
                    warn!(
                        "Account {}: Failed to sync label '{}'. Error: {:#?}. Removing label entry.",
                        account.id, &label.name, e
                    );
                    if let Err(del_err) = GmailLabels::delete(label.id).await {
                        error!(
                            "Account {}: Failed to delete label '{}' after sync error: {}",
                            account.id, &label.name, del_err
                        );
                    }
                }
            },
        }
    }
    Ok(None)
}
