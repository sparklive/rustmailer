// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::sync::Arc;

use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::error;

use crate::{
    modules::{
        account::{status::AccountRunningState, v2::AccountV2},
        cache::vendor::gmail::model::messages::MessageMeta,
        cache::vendor::gmail::sync::{
            client::GmailClient, envelope::GmailEnvelope, labels::GmailLabels,
        },
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error,
};

const ENVELOPE_BATCH_SIZE: u32 = 500;

pub async fn fetch_and_save_since_date(
    account: &AccountV2,
    date: &str,
    label: &GmailLabels,
    initial: bool,
) -> RustMailerResult<(usize, Option<String>)> {
    let page_size = ENVELOPE_BATCH_SIZE;
    // let total_batches = total.div_ceil(page_size); // Calculate total number of batches, useful for tracking sync progress on UI
    let mut inserted_count = 0;
    let account_id = account.id;
    let use_proxy = account.use_proxy;
    // Gmail API pagination relies on pageToken.
    // Each page returns message IDs, and we still need to fetch message details individually.
    let mut page_token: Option<String> = None;
    let mut page = 1; // Used only for tracking sync progress
    let semaphore = Arc::new(Semaphore::new(10));
    let mut max_history_id = None;
    loop {
        let resp = GmailClient::list_messages(
            account_id,
            use_proxy,
            &label.label_id,
            page_token,
            Some(date),
            ENVELOPE_BATCH_SIZE,
        )
        .await?;
        // The total number of messages can only be retrieved via an API query
        if page == 1 && initial {
            let total = resp.result_size_estimate;
            if let Some(total) = total {
                let total_batches = (total as u32).div_ceil(page_size);
                AccountRunningState::set_initial_current_syncing_folder(
                    account_id,
                    label.name.clone(),
                    total_batches,
                )
                .await?;
            }
        }
        // Update page_token returned by Gmail API
        page_token = resp.next_page_token;
        // Concurrently fetch message details for this page, with concurrency limited to 10
        if let Some(messages) = resp.messages {
            let mut batch_messages = Vec::with_capacity(ENVELOPE_BATCH_SIZE as usize);
            if initial {
                AccountRunningState::set_current_sync_batch_number(account_id, page).await?;
            }
            let mut handles: Vec<JoinHandle<RustMailerResult<MessageMeta>>> = Vec::new();
            for msg in messages {
                match semaphore.clone().acquire_owned().await {
                    Ok(permit) => {
                        let handle: JoinHandle<RustMailerResult<MessageMeta>> =
                            tokio::spawn(async move {
                                // Permit will be released automatically when this task finishes
                                let _permit = permit;
                                GmailClient::get_messages(account_id, use_proxy, &msg.id).await
                            });

                        handles.push(handle);
                    }
                    Err(err) => error!("Failed to acquire semaphore permit, error: {:#?}", err),
                }
            }
            for handle in handles {
                match handle.await {
                    Ok(Ok(meta)) => batch_messages.push(meta),
                    Ok(Err(e)) => return Err(e),
                    Err(join_err) => {
                        return Err(raise_error!(
                            format!("tokio task join error: {:?}", join_err),
                            ErrorCode::InternalError
                        ));
                    }
                }
            }
            // All message details for this batch are collected, now convert and save them
            let envelopes: Vec<GmailEnvelope> = batch_messages
                .into_iter()
                .map(|m| {
                    let mut envelope: GmailEnvelope = m.try_into()?;
                    envelope.account_id = account_id;
                    envelope.label_id = label.id;
                    envelope.label_name = label.name.clone();
                    Ok(envelope)
                })
                .collect::<RustMailerResult<Vec<GmailEnvelope>>>()?;
            inserted_count += envelopes.len();
            max_history_id = compute_max_history_id(&envelopes);
            GmailEnvelope::save_envelopes(envelopes).await?;
        }
        // Break if API response has no next page
        if page_token.is_none() {
            break;
        }
        page += 1;
    }
    Ok((inserted_count, max_history_id))
}

pub async fn fetch_and_save_full_label(
    account: &AccountV2,
    label: &GmailLabels,
    total: u32,
    initial: bool,
) -> RustMailerResult<(usize, Option<String>)> {
    let page_size = ENVELOPE_BATCH_SIZE;
    let total_batches = total.div_ceil(page_size); // Calculate total number of batches, useful for tracking sync progress on UI
    let mut inserted_count = 0;
    let account_id = account.id;
    let use_proxy = account.use_proxy;
    // If this is the first synchronization, set the initial state for the account
    if initial {
        AccountRunningState::set_initial_current_syncing_folder(
            account_id,
            label.name.clone(),
            total_batches,
        )
        .await?;
    }
    // Gmail API pagination relies on pageToken.
    // Each page returns message IDs, and we still need to fetch message details individually.
    let mut page_token: Option<String> = None;
    let mut page = 1; // Used only for tracking sync progress
    let semaphore = Arc::new(Semaphore::new(10));
    let mut max_history_id = None;
    loop {
        let resp = GmailClient::list_messages(
            account_id,
            use_proxy,
            &label.label_id,
            page_token,
            None,
            ENVELOPE_BATCH_SIZE,
        )
        .await?;
        // Update page_token returned by Gmail API
        page_token = resp.next_page_token;
        // Concurrently fetch message details for this page, with concurrency limited to 10
        if let Some(messages) = resp.messages {
            let mut batch_messages = Vec::with_capacity(ENVELOPE_BATCH_SIZE as usize);
            if initial {
                AccountRunningState::set_current_sync_batch_number(account_id, page).await?;
            }
            let mut handles: Vec<JoinHandle<RustMailerResult<MessageMeta>>> = Vec::new();
            for msg in messages {
                match semaphore.clone().acquire_owned().await {
                    Ok(permit) => {
                        let handle: JoinHandle<RustMailerResult<MessageMeta>> =
                            tokio::spawn(async move {
                                // Permit will be released automatically when this task finishes
                                let _permit = permit;
                                GmailClient::get_messages(account_id, use_proxy, &msg.id).await
                            });

                        handles.push(handle);
                    }
                    Err(err) => error!("Failed to acquire semaphore permit, error: {:#?}", err),
                }
            }
            for handle in handles {
                match handle.await {
                    Ok(Ok(meta)) => batch_messages.push(meta),
                    Ok(Err(e)) => return Err(e),
                    Err(join_err) => {
                        return Err(raise_error!(
                            format!("tokio task join error: {:?}", join_err),
                            ErrorCode::InternalError
                        ));
                    }
                }
            }
            // All message details for this batch are collected, now convert and save them
            let envelopes: Vec<GmailEnvelope> = batch_messages
                .into_iter()
                .map(|m| {
                    let mut envelope: GmailEnvelope = m.try_into()?;
                    envelope.account_id = account_id;
                    envelope.label_id = label.id;
                    envelope.label_name = label.name.clone();
                    Ok(envelope)
                })
                .collect::<RustMailerResult<Vec<GmailEnvelope>>>()?;
            inserted_count += envelopes.len();
            max_history_id = compute_max_history_id(&envelopes);
            GmailEnvelope::save_envelopes(envelopes).await?;
        }
        // Break if API response has no next page
        if page_token.is_none() {
            break;
        }
        page += 1;
    }
    Ok((inserted_count, max_history_id))
}

fn max_history_id_fallback(a: &str, b: &str) -> String {
    // Try to parse as u64
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(a_num), Ok(b_num)) => {
            if a_num >= b_num {
                a.to_string()
            } else {
                b.to_string()
            }
        }
        // If parsing fails, fall back to length + lexicographical comparison
        _ => {
            if a.len() > b.len() {
                a.to_string()
            } else if b.len() > a.len() {
                b.to_string()
            } else {
                // Same length, compare lexicographically
                if a >= b {
                    a.to_string()
                } else {
                    b.to_string()
                }
            }
        }
    }
}

fn compute_max_history_id(envelopes: &[GmailEnvelope]) -> Option<String> {
    envelopes
        .iter()
        .map(|e| e.history_id.as_str())
        .fold(None, |max_id, curr| {
            Some(match max_id {
                Some(m) => max_history_id_fallback(m.as_str(), curr),
                None => curr.to_string(),
            })
        })
}
