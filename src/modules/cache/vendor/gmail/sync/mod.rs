// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

pub mod client;
pub mod envelope;
pub mod flow;
pub mod history;
pub mod labels;
pub mod rebuild;
pub mod sync_labels;

use std::time::Instant;

use ahash::AHashSet;
use tracing::info;

use crate::modules::{
    account::{entity::MailerType, status::AccountRunningState, v2::AccountV2},
    cache::{
        imap::{
            address::AddressEntity,
            sync::sync_type::{determine_sync_type, SyncType},
            thread::EmailThread,
        },
        vendor::gmail::sync::{
            envelope::GmailEnvelope,
            history::handle_history,
            labels::{GmailCheckPoint, GmailLabels},
            rebuild::{rebuild_cache, rebuild_cache_since_date, rebuild_single_label_cache},
            sync_labels::get_sync_labels,
        },
    },
    error::RustMailerResult,
    hook::{
        channel::{Event, EVENT_CHANNEL},
        events::{payload::AccountChange, EventPayload, EventType, RustMailerEvent},
        task::EventHookTask,
    },
    utils::mailbox_id,
};

pub async fn execute_gmail_sync(account: &AccountV2) -> RustMailerResult<()> {
    assert!(
        matches!(account.mailer_type, MailerType::GmailApi),
        "Bug: Unexpected mailer type, expected GmailApi, found: {:?}",
        account.mailer_type
    );

    let sync_type = determine_sync_type(account).await?;
    if matches!(sync_type, SyncType::SkipSync) {
        return Ok(());
    }

    let remote_labels = get_sync_labels(account).await?;
    let remote_labels: Vec<GmailLabels> = remote_labels
        .into_iter()
        .map(|label| {
            let mut label: GmailLabels = label.into();
            label.account_id = account.id;
            label.id = mailbox_id(account.id, &label.label_id);
            label
        })
        .collect();

    let local_labels = GmailLabels::list_all(account.id).await?;
    // How to determine if a rebuild is needed?
    // Simplified rule: if the local label does not exist, trigger a rebuild.
    // We do not check how many local message metadata entries exist,
    // since that would be expensive.
    let local_checkpoints = GmailCheckPoint::list_all(account.id).await?;
    if should_rebuild_cache(account, local_labels.len(), local_checkpoints.len()).await? {
        AccountRunningState::set_initial_sync_folders(
            account.id,
            remote_labels
                .iter()
                .map(|label| label.name.clone())
                .collect(),
        )
        .await?;
    
        match &account.date_since {
            Some(date_since) => {
                rebuild_cache_since_date(account, &remote_labels, date_since).await?;
            }
            None => {
                rebuild_cache(account, &remote_labels).await?;
            }
        }
        AccountRunningState::set_initial_sync_completed(account.id).await?;
        if EventHookTask::event_watched(account.id, EventType::AccountFirstSyncCompleted).await? {
            EVENT_CHANNEL
                .queue(Event::new(
                    account.id,
                    &account.email,
                    RustMailerEvent::new(
                        EventType::AccountFirstSyncCompleted,
                        EventPayload::AccountFirstSyncCompleted(AccountChange {
                            account_id: account.id,
                            account_email: account.email.clone(),
                        }),
                    ),
                ))
                .await;
        }
        return Ok(());
    }
    handle_history(account, &local_labels, &remote_labels).await?;

    let deleted_labels = find_deleted_labels(&local_labels, &remote_labels);
    let missing_labels = find_missing_labels(&local_labels, &remote_labels);

    if !deleted_labels.is_empty() {
        info!(
            "Account {}: Detected {} mailboxes missing from the IMAP server (not found in the LSUB response). \
            Now cleaning up these mailboxes and their associated metadata locally.",
            account.id, deleted_labels.len()
        );
        cleanup_deleted_labels(account, &deleted_labels).await?;
    }

    if !missing_labels.is_empty() {
        GmailLabels::batch_insert(&missing_labels).await?;
        for label in &missing_labels {
            rebuild_single_label_cache(account, label).await?;
        }
    }
    Ok(())
}

pub async fn should_rebuild_cache(
    account: &AccountV2,
    local_labels_count: usize,
    local_checkpoints_count: usize,
) -> RustMailerResult<bool> {
    // If both local labels and checkpoint exist, no rebuild is needed.
    if local_labels_count > 0 && local_checkpoints_count > 0 {
        return Ok(false);
    }
    // If there are local mailboxes but no checkpoints, clear the mailboxes.
    if local_labels_count > 0 {
        let mailboxes = GmailLabels::list_all(account.id).await?;
        GmailLabels::batch_delete(mailboxes).await?;
    }
    if local_checkpoints_count > 0 {
        //这个要清理，清理掉本地缓存的所有信息，包括关联的索引信息，比如thread, checkpoint也是
        //EnvelopeFlagsManager::clean_account(account.id).await?
        GmailCheckPoint::clean(account.id).await?;
    }
    // If either remote mailboxes or local envelopes were missing, cache rebuild is required.
    Ok(true)
}

pub fn find_deleted_labels(
    local_labels: &[GmailLabels],
    remote_labels: &[GmailLabels],
) -> Vec<GmailLabels> {
    let remote_ids: AHashSet<_> = remote_labels.iter().map(|l| &l.id).collect();

    local_labels
        .iter()
        .filter(|l| !remote_ids.contains(&l.id))
        .cloned()
        .collect()
}

pub fn find_missing_labels(
    local_labels: &[GmailLabels],
    remote_labels: &[GmailLabels],
) -> Vec<GmailLabels> {
    let local_ids: AHashSet<_> = local_labels.iter().map(|l| &l.id).collect();

    remote_labels
        .iter()
        .filter(|l| !local_ids.contains(&l.id))
        .cloned()
        .collect()
}

async fn cleanup_deleted_labels(
    account: &AccountV2,
    deleted_labels: &[GmailLabels],
) -> RustMailerResult<()> {
    let start_time = Instant::now();
    for label in deleted_labels {
        GmailEnvelope::clean_label_envelopes(account.id, label.id).await?;
        AddressEntity::clean_mailbox_envelopes(account.id, label.id).await?;
        EmailThread::clean_mailbox_envelopes(account.id, label.id).await?;
    }
    GmailLabels::batch_delete(deleted_labels.to_vec()).await?;
    let elapsed_time = start_time.elapsed().as_secs();
    info!(
        "Cleanup deleted GmailLabels completed: {} seconds elapsed.",
        elapsed_time
    );
    Ok(())
}

async fn cleanup_single_label(account: &AccountV2, label: &GmailLabels) -> RustMailerResult<()> {
    let start_time = Instant::now();
    GmailEnvelope::clean_label_envelopes(account.id, label.id).await?;
    AddressEntity::clean_mailbox_envelopes(account.id, label.id).await?;
    EmailThread::clean_mailbox_envelopes(account.id, label.id).await?;
    GmailLabels::delete(label.id).await?;
    let elapsed_time = start_time.elapsed().as_secs();
    info!(
        "Cleanup GmailLabels completed: {} seconds elapsed.",
        elapsed_time
    );
    Ok(())
}
