// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::sync::Arc;
use tracing::{debug, warn};

use crate::{
    modules::{
        account::v2::AccountV2,
        cache::vendor::gmail::model::labels::{Label, LabelDetail},
        cache::{
            imap::sync::sync_folders::detect_mailbox_changes,
            vendor::gmail::sync::client::GmailClient,
        },
        error::{code::ErrorCode, RustMailerError, RustMailerResult},
    },
    raise_error,
};

pub async fn get_sync_labels(account: &AccountV2) -> RustMailerResult<Vec<LabelDetail>> {
    let visible_labels = GmailClient::list_visible_labels(account.id, account.use_proxy).await?;
    debug!(
        "Account {}: Retrieved {} visible labels from Gmail API: {:?}",
        account.id,
        visible_labels.len(),
        visible_labels.iter().map(|l| &l.name).collect::<Vec<_>>()
    );
    // Exclude all labels that cannot retrieve messages via the message list,
    // since we use the message API to fetch message details.
    if visible_labels.is_empty() {
        warn!(
            "Account {}: No visible labels returned from Gmail API.",
            account.id
        );
        return Err(
            raise_error!(
                format!(
                    "No visible labels returned from Gmail API for account {}. This is unexpected and may indicate an issue with the Gmail API or data.",
                    account.id
                ),
                ErrorCode::InternalError
            )
        );
    }
    // Detect label changes through this method and send notifications.
    detect_mailbox_changes(
        account,
        visible_labels
            .iter()
            .map(|label| label.name.clone())
            .collect(),
    )
    .await?;

    //sync_folders stores the mailbox names for IMAP accounts, whereas for Gmail API accounts it stores the label IDs.
    let subscribed = &account.sync_folders;
    debug!(
        "Account {}: Current subscribed sync folders: {:?}",
        account.id, subscribed
    );
    // Filter labels according to the subscription list; matched_labels will not include any labels outside of it.
    let mut matched_labels: Vec<&Label> = if !subscribed.is_empty() {
        visible_labels
            .iter()
            .filter(|label| subscribed.contains(&label.id))
            .collect()
    } else {
        Vec::new()
    };
    debug!(
        "Account {}: Matched labels after subscription filter: {:?}",
        account.id,
        matched_labels.iter().map(|l| &l.name).collect::<Vec<_>>()
    );
    // If there are no subscriptions, default to the two special labels: INBOX and SENT
    if matched_labels.is_empty() {
        matched_labels = visible_labels
            .iter()
            .filter(|label| label.id == "INBOX" || label.id == "SENT")
            .collect();

        debug!(
            "Account {}: Matched labels after default INBOX/SENT filter: {:?}",
            account.id,
            matched_labels.iter().map(|l| &l.name).collect::<Vec<_>>()
        );
        if !matched_labels.is_empty() {
            let sync_folders: Vec<String> = matched_labels.iter().map(|n| n.id.clone()).collect();
            AccountV2::update_sync_folders(account.id, sync_folders).await?;
        } else {
            warn!("Account {}: No visible labels found from Gmail API. This is unexpected — Gmail API should at least provide INBOX.", account.id);
            return Err(
                raise_error!(
                    format!("No visible labels found for account {} via Gmail API. This is unexpected — Gmail API should at least provide INBOX.", &account.id), 
                    ErrorCode::InternalError
                )
            );
        }
    }
    retrieve_label_metadata(account, matched_labels).await
}

pub async fn retrieve_label_metadata(
    account: &AccountV2,
    labels: impl IntoIterator<Item = &Label>,
) -> RustMailerResult<Vec<LabelDetail>> {
    let mut tasks = Vec::new();

    let account = Arc::new(account.clone());
    for label in labels.into_iter() {
        let label_id = label.id.clone();
        let account = account.clone();
        let task: tokio::task::JoinHandle<Result<LabelDetail, RustMailerError>> =
            tokio::spawn(async move {
                GmailClient::get_label(account.id, account.use_proxy, label_id.as_str()).await
            });
        tasks.push(task);
    }

    let mut details = Vec::new();

    for task in tasks {
        match task.await {
            Ok(Ok(detail)) => details.push(detail),
            Ok(Err(err)) => return Err(err),
            Err(e) => return Err(raise_error!(format!("{:#?}", e), ErrorCode::InternalError)),
        }
    }
    Ok(details)
}
