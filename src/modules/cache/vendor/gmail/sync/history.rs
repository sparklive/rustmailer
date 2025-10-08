// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use ahash::{AHashMap, AHashSet, HashSet};
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::{
    modules::{
        account::v2::AccountV2,
        cache::vendor::gmail::{
            model::history::History,
            sync::{
                cleanup_single_label,
                client::GmailClient,
                envelope::GmailEnvelope,
                flow::max_history_id,
                labels::{GmailCheckPoint, GmailLabels},
                rebuild::rebuild_single_label_cache,
            },
        },
        error::{code::ErrorCode, RustMailerError, RustMailerResult},
        hook::{
            channel::{Event, EVENT_CHANNEL},
            events::{
                payload::{Attachment, EmailAddedToFolder, EmailFlagsChanged},
                EventPayload, EventType, RustMailerEvent,
            },
            task::EventHookTask,
        },
        message::content::FullMessageContent,
    },
    raise_error,
};

pub async fn handle_history(
    account: &AccountV2,
    local_labels: &[GmailLabels],
    remote_labels: &[GmailLabels],
) -> RustMailerResult<()> {
    let account_id = account.id;
    let use_proxy = account.use_proxy.clone();
    let remote_labels = find_existing_remote_labels(local_labels, remote_labels);
    let checkpoint = GmailCheckPoint::get(account_id).await?;
    let mut history_ids = Vec::with_capacity(remote_labels.len());
    for remote in remote_labels {
        let mut page_token = None;
        loop {
            let mut list = match GmailClient::list_history(
                account_id,
                use_proxy.clone(),
                &remote.label_id,
                &checkpoint.history_id,
                page_token.as_deref(),
                100, // 100 items per page
            )
            .await
            {
                Ok(list) => list,
                Err(error) => match error {
                    RustMailerError::Generic {
                        message,
                        location: _,
                        code,
                    } => {
                        if code == ErrorCode::GmailApiInvalidHistoryId {
                            let history_id = handle_invalid_history_id(account, &remote).await?;
                            if let Some(history_id) = history_id {
                                history_ids.push(history_id);
                            }
                            break;
                        } else {
                            return Err(raise_error!(message, code));
                        }
                    }
                },
            };
            page_token = list.next_page_token.take();

            let history_list: Vec<History> = list
                .history
                .into_iter()
                .filter(|h| h.has_changes())
                .collect();
            apply_history(account, &remote, history_list).await?;
            if page_token.is_none() {
                history_ids.push(list.history_id);
                break;
            }
        }
        GmailLabels::upsert(remote).await?;
    }
    let max = max_history_id(&history_ids);
    if let Some(history_id) = max {
        let checkpoint = GmailCheckPoint::new(account_id, history_id.to_string());
        checkpoint.save().await?;
    }
    Ok(())
}

pub fn find_existing_remote_labels(
    local_labels: &[GmailLabels],
    remote_labels: &[GmailLabels],
) -> Vec<GmailLabels> {
    let local_ids: AHashSet<_> = local_labels.iter().map(|l| &l.id).collect();

    remote_labels
        .iter()
        .filter(|remote| local_ids.contains(&remote.id))
        .cloned()
        .collect()
}

#[derive(Debug, Default)]
struct LabelChange {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

pub async fn apply_history(
    account: &AccountV2,
    label: &GmailLabels,
    history_list: Vec<History>,
) -> RustMailerResult<()> {
    for history in history_list {
        let mut label_changes: AHashMap<String, LabelChange> = AHashMap::new();
        // -- labels_added
        for item in history.labels_added {
            let current =
                GmailEnvelope::find(account.id, label.id, item.message.id.as_str()).await?;
            match current {
                Some(mut current) => {
                    let mut merged: HashSet<String> = current.label_ids.into_iter().collect();
                    let mut actually_added = Vec::new();
                    if let Some(to_add) = &item.label_ids {
                        for l in to_add {
                            if !merged.contains(l) {
                                actually_added.push(l.clone());
                            }
                        }
                        merged.extend(to_add.iter().cloned());
                    }
                    if !actually_added.is_empty() {
                        let entry = label_changes
                            .entry(item.message.id.clone())
                            .or_insert_with(|| LabelChange::default());
                        entry.added.extend(actually_added);
                    }
                    current.label_ids = merged.into_iter().collect();
                    GmailEnvelope::upsert(current).await?;
                }
                None => {
                    warn!(
                        "Message {} not found in local cache, cannot merge labels.",
                        item.message.id
                    );
                }
            }
        }
        // -- labels_removed
        for item in history.labels_removed {
            let current =
                GmailEnvelope::find(account.id, label.id, item.message.id.as_str()).await?;
            match current {
                Some(mut current) => {
                    if let Some(to_remove) = &item.label_ids {
                        let mut actually_removed = Vec::new();
                        if to_remove.contains(&label.id.to_string()) {
                            GmailEnvelope::delete(account.id, label.id, &current.id).await?;
                        } else {
                            current.label_ids.retain(|id| {
                                let keep = !to_remove.contains(id);
                                if !keep {
                                    actually_removed.push(id.clone());
                                }
                                keep
                            });
                            GmailEnvelope::upsert(current).await?;
                        }
                        if !actually_removed.is_empty() {
                            let entry = label_changes
                                .entry(item.message.id.clone())
                                .or_insert_with(|| LabelChange::default());
                            entry.removed.extend(actually_removed);
                        }
                    }
                }
                None => {
                    warn!(
                        "Message {} not found in local cache, cannot merge labels.",
                        item.message.id
                    );
                }
            }
        }

        if !label_changes.is_empty() {
            if !account.minimal_sync()
                && EventHookTask::is_watching_email_flags_changed(account.id).await?
            {
                for entry in label_changes {
                    if let Some(current) =
                        GmailEnvelope::find(account.id, label.id, entry.0.as_str()).await?
                    {
                        EVENT_CHANNEL
                            .queue(Event::new(
                                account.id,
                                &account.email,
                                RustMailerEvent::new(
                                    EventType::EmailFlagsChanged,
                                    EventPayload::EmailFlagsChanged(EmailFlagsChanged {
                                        account_id: account.id,
                                        account_email: account.email.clone(),
                                        mailbox_name: current.label_name,
                                        uid: None,
                                        from: current.from,
                                        to: current.to,
                                        message_id: current.message_id,
                                        subject: current.subject,
                                        internal_date: Some(current.internal_date),
                                        date: current.date,
                                        flags_added: entry.1.added,
                                        flags_removed: entry.1.removed,
                                        mid: Some(entry.0),
                                    }),
                                ),
                            ))
                            .await;
                    }
                }
            }
        }

        let len = history.messages_added.len();
        let mut handles: Vec<JoinHandle<Option<GmailEnvelope>>> = Vec::with_capacity(len);
        // -- messages_added
        let account_id = account.id;
        let use_proxy = account.use_proxy;
        for item in history.messages_added {
            let label = label.clone();
            handles.push(tokio::spawn(async move {
                if !item.message.label_ids.contains(&label.label_id) {
                    return None;
                }
                let message_data =
                    match GmailClient::get_message(account_id, use_proxy.clone(), &item.message.id)
                        .await
                    {
                        Ok(msg) => msg,
                        Err(_) => return None,
                    };
                if !message_data.label_ids.contains(&label.label_id) {
                    return None;
                }

                let mut envelope: GmailEnvelope = match message_data.try_into() {
                    Ok(env) => env,
                    Err(_) => return None,
                };
                envelope.account_id = account_id;
                envelope.label_id = label.id;
                envelope.label_name = label.name.clone();

                Some(envelope)
            }));
        }

        let mut messages_added = Vec::with_capacity(len);
        for handle in handles {
            if let Ok(Some(envelope)) = handle.await {
                messages_added.push(envelope);
            }
        }
        // save to local envelope cache and build some index
        if !messages_added.is_empty() {
            info!(
                "Gmail Api Account {} synced {} new messages in label '{}'",
                account.id,
                messages_added.len(),
                &label.name
            );
            GmailEnvelope::save_envelopes(messages_added.clone()).await?;
            if EventHookTask::is_watching_email_add_event(account.id).await? {
                dispatch_new_email_notification(account, messages_added).await?;
            }
        }
        //Deletion events are temporarily not handled
        // for item in history.messages_deleted {
        //     if item.message.label_ids.contains(&label.label_id) {
        //         let mid = item.message.id;
        //         let message_data =
        //             GmailClient::get_messages(account_id, use_proxy.clone(), &mid).await?;
        //         // We can directly delete it here
        //         if message_data.label_ids.contains(&label.label_id) {
        //             GmailEnvelope::delete(account_id, label.id, &message_data.id).await?;
        //         }
        //     }
        // }
    }

    Ok(())
}

async fn dispatch_new_email_notification(
    account: &AccountV2,
    messages: Vec<GmailEnvelope>,
) -> RustMailerResult<()> {
    let label_map = GmailClient::label_map(account.id, account.use_proxy).await?;
    for message in messages {
        let full_message =
            GmailClient::get_full_messages(account.id, account.use_proxy, &message.id).await?;
        let message_content: FullMessageContent = full_message.try_into()?;
        let mut envelope = message.into_envelope(&label_map);
        envelope.thread_id = envelope.compute_thread_id();
        EVENT_CHANNEL
            .queue(Event::new(
                account.id,
                &account.email,
                RustMailerEvent::new(
                    EventType::EmailAddedToFolder,
                    EventPayload::EmailAddedToFolder(EmailAddedToFolder {
                        account_id: account.id,
                        account_email: account.email.clone(),
                        mailbox_name: envelope.mailbox_name.clone(),
                        id: envelope.id,
                        internal_date: envelope.internal_date,
                        date: envelope.date,
                        from: envelope.from,
                        subject: envelope.subject,
                        to: envelope.to,
                        size: envelope.size,
                        flags: envelope
                            .flags
                            .map(|f| f.into_iter().map(|f| f.to_string()).collect())
                            .unwrap_or_default(),
                        cc: envelope.cc,
                        bcc: envelope.bcc,
                        in_reply_to: envelope.in_reply_to,
                        sender: envelope.sender,
                        message_id: envelope.message_id,
                        message: message_content,
                        thread_name: envelope.thread_name,
                        reply_to: envelope.reply_to,
                        attachments: envelope
                            .attachments
                            .as_ref()
                            .map(|atts| atts.iter().cloned().map(Attachment::from).collect()),
                        thread_id: envelope.thread_id,
                        labels: envelope.labels,
                    }),
                ),
            ))
            .await;
    }
    Ok(())
}

async fn handle_invalid_history_id(
    account: &AccountV2,
    label: &GmailLabels,
) -> RustMailerResult<Option<String>> {
    info!(
        "Account {}: Invalid history ID detected for label '{}'. Rebuilding local state...",
        account.id, label.name
    );
    cleanup_single_label(account, label).await?;
    info!(
        "Account {}: Cleaned up local data for label '{}'",
        account.id, label.name
    );
    GmailLabels::upsert(label.clone()).await?;
    info!(
        "Account {}: Upserted label '{}' into local database",
        account.id, label.name
    );
    rebuild_single_label_cache(account, label).await
}
