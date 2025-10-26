use tracing::info;

use crate::modules::{
    account::{migration::AccountModel, status::AccountRunningState},
    cache::{
        imap::{address::AddressEntity, thread::EmailThread},
        sync_type::{determine_sync_type, SyncType},
        vendor::outlook::sync::{
            delta::FolderDeltaLink,
            envelope::OutlookEnvelope,
            folders::OutlookFolder,
            rebuild::{rebuild_cache, rebuild_cache_since_date},
            sync_folders::get_sync_folders,
        },
    },
    error::{RustMailerError, RustMailerResult},
    hook::{
        channel::{Event, EVENT_CHANNEL},
        events::{payload::AccountChange, EventPayload, EventType, RustMailerEvent},
        task::EventHookTask,
    },
    utils::mailbox_id,
};

pub mod client;
pub mod delta;
pub mod envelope;
pub mod flow;
pub mod folders;
pub mod rebuild;
pub mod sync_folders;
pub async fn execute_outlook_sync(account: &AccountModel) -> RustMailerResult<()> {
    // assert!(
    //     matches!(account.mailer_type, MailerType::GraphApi),
    //     "Bug: Unexpected mailer type, expected GraphApi, found: {:?}",
    //     account.mailer_type
    // );

    let sync_type = determine_sync_type(account).await?;
    if matches!(sync_type, SyncType::SkipSync) {
        return Ok(());
    }

    let remote_folders = get_sync_folders(account).await?;
    let remote_folders: Vec<OutlookFolder> = remote_folders
        .into_iter()
        .map(|folder| {
            let mut f: OutlookFolder = folder.try_into()?; // may fail
            f.account_id = account.id;
            f.id = mailbox_id(account.id, &f.folder_id);
            Ok::<OutlookFolder, RustMailerError>(f)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let local_folders = OutlookFolder::list_all(account.id).await?;
    let delta_links = FolderDeltaLink::get_by_account(account.id).await?;

    if should_rebuild_cache(account, &local_folders, !delta_links.is_empty()).await? {
        AccountRunningState::set_initial_sync_folders(
            account.id,
            remote_folders.iter().map(|f| f.name.clone()).collect(),
        )
        .await?;
        match &account.date_since {
            Some(date_since) => {
                rebuild_cache_since_date(account, &remote_folders, date_since).await?;
            }
            None => {
                rebuild_cache(account, &remote_folders).await?;
            }
        }
        AccountRunningState::set_initial_sync_completed(account.id).await?;
        if EventHookTask::is_watching_account_first_sync_completed(account.id).await? {
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
    todo!()
}

pub async fn should_rebuild_cache(
    account: &AccountModel,
    local_folders: &[OutlookFolder],
    has_delta: bool,
) -> RustMailerResult<bool> {
    // If both local labels and checkpoint exist, no rebuild is needed.
    if !local_folders.is_empty() && has_delta {
        return Ok(false);
    }

    info!(
        account_id = account.id,
        folder_count = local_folders.len(),
        "Rebuilding cache: cleaning local folders and checkpoints"
    );

    if !local_folders.is_empty() {
        OutlookFolder::batch_delete(local_folders.to_vec()).await?;
    }
    if has_delta {
        FolderDeltaLink::clean(account.id).await?;
    }
    OutlookEnvelope::clean_account(account.id).await?;
    AddressEntity::clean_account(account.id).await?;
    EmailThread::clean_account(account.id).await?;
    info!(account_id = account.id, "Cache cleaning completed");
    Ok(true)
}
