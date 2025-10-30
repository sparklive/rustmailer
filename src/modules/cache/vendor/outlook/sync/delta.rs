use ahash::AHashSet;
use itertools::Itertools;
use native_db::*;
use native_model::{native_model, Model};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

use crate::{
    modules::{
        account::migration::AccountModel,
        cache::vendor::outlook::{
            model::DeltaResponse,
            sync::{client::OutlookClient, envelope::OutlookEnvelope, folders::OutlookFolder},
        },
        database::{
            async_find_impl, batch_delete_impl, filter_by_secondary_key_impl, manager::DB_MANAGER,
            upsert_impl,
        },
        error::{code::ErrorCode, RustMailerResult},
        hook::http::HttpClient,
        utils::mailbox_id,
    },
    raise_error, utc_now,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
#[native_model(id = 10, version = 1)]
#[native_db]
pub struct FolderDeltaLink {
    #[primary_key]
    pub id: u64,
    #[secondary_key]
    pub account_id: u64,
    pub link: String,
    pub updated_at: i64,
}

impl FolderDeltaLink {
    pub async fn upsert(account_id: u64, folder_id: &str, link: &str) -> RustMailerResult<()> {
        let id = mailbox_id(account_id, folder_id);
        let item = Self {
            id,
            account_id,
            link: link.to_string(),
            updated_at: utc_now!(),
        };
        upsert_impl(DB_MANAGER.envelope_db(), item).await
    }

    // pub async fn delete(id: u64) -> RustMailerResult<()> {
    //     delete_impl(DB_MANAGER.envelope_db(), move |rw| {
    //         rw.get()
    //             .primary::<FolderDeltaLink>(id)
    //             .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
    //             .ok_or_else(|| {
    //                 raise_error!("folder delta link missing".into(), ErrorCode::InternalError)
    //             })
    //     })
    //     .await
    // }

    pub async fn get(account_id: u64, folder_id: &str) -> RustMailerResult<Self> {
        let id = mailbox_id(account_id, folder_id);
        let result = async_find_impl::<FolderDeltaLink>(DB_MANAGER.envelope_db(), id).await?;
        let result = result.ok_or_else(|| {
            raise_error!(
                format!(
                    "Folder delta link '{}' not found for account {}",
                    folder_id, account_id
                ),
                ErrorCode::MailBoxNotCached
            )
        })?;
        Ok(result)
    }

    pub async fn get_by_account(account_id: u64) -> RustMailerResult<Vec<FolderDeltaLink>> {
        filter_by_secondary_key_impl(
            DB_MANAGER.envelope_db(),
            FolderDeltaLinkKey::account_id,
            account_id,
        )
        .await
    }

    pub async fn clean(account_id: u64) -> RustMailerResult<()> {
        batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
            let links: Vec<FolderDeltaLink> = rw
                .scan()
                .secondary::<FolderDeltaLink>(FolderDeltaLinkKey::account_id)
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .start_with(account_id)
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .try_collect()
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
            Ok(links)
        })
        .await?;
        Ok(())
    }
}

pub async fn handle_delta(
    account: &AccountModel,
    local_folders: &[OutlookFolder],
    remote_folders: &[OutlookFolder],
) -> RustMailerResult<()> {
    let account_id = account.id;
    let use_proxy = account.use_proxy.clone();
    let remote_folders = find_existing_remote_folders(local_folders, remote_folders);
    for remote in remote_folders {
        let mut url = FolderDeltaLink::get(account_id, &remote.folder_id)
            .await?
            .link;
        let client = HttpClient::new(use_proxy).await?;
        let access_token = OutlookClient::get_access_token(account_id).await?;
        let mut batch = Vec::new();
        loop {
            let value = client.get(url.as_str(), &access_token).await?;
            let resp = serde_json::from_value::<DeltaResponse>(value).map_err(|e| {
                raise_error!(
                    format!(
                        "Failed to deserialize Graph API response into MessageListResponse: {:#?}. Possible model mismatch or API change.",
                        e
                    ),
                    ErrorCode::InternalError
                )
            })?;
            if let Some(items) = resp.value {
                for item in items {
                    if item.removed.is_none() {
                        let message =
                            OutlookClient::get_message(account_id, use_proxy, &item.id).await?;
                        let mut envelope: OutlookEnvelope = message.try_into()?;
                        envelope.account_id = account_id;
                        envelope.folder_id = remote.id;
                        envelope.folder_name = remote.name.clone();
                        batch.push(envelope);
                    }
                }
            }
            if let Some(next_link) = resp.next_link {
                url = next_link;
            } else if let Some(delta_link) = resp.delta_link {
                let new_delta_link = delta_link;
                FolderDeltaLink::upsert(account_id, &remote.folder_id, &new_delta_link).await?;
                break;
            } else {
                return Err(raise_error!(format!(
                    "neither @odata.nextLink nor @odata.deltaLink found in Graph API response at URL={url}"
                ), ErrorCode::InternalError));
            }
        }
        OutlookEnvelope::save_envelopes(batch).await?;
        OutlookFolder::upsert(remote).await?;
    }
    Ok(())
}

pub fn find_existing_remote_folders(
    local_folders: &[OutlookFolder],
    remote_folders: &[OutlookFolder],
) -> Vec<OutlookFolder> {
    let local_ids: AHashSet<_> = local_folders.iter().map(|l| &l.id).collect();
    remote_folders
        .iter()
        .filter(|remote| local_ids.contains(&remote.id))
        .cloned()
        .collect()
}
