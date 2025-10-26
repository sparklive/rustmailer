use itertools::Itertools;
use native_db::*;
use native_model::{native_model, Model};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

use crate::{
    modules::{
        database::{
            async_find_impl, batch_delete_impl, delete_impl, filter_by_secondary_key_impl,
            manager::DB_MANAGER, upsert_impl,
        },
        error::{code::ErrorCode, RustMailerResult},
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

    pub async fn delete(id: u64) -> RustMailerResult<()> {
        delete_impl(DB_MANAGER.envelope_db(), move |rw| {
            rw.get()
                .primary::<FolderDeltaLink>(id)
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .ok_or_else(|| {
                    raise_error!("folder delta link missing".into(), ErrorCode::InternalError)
                })
        })
        .await
    }

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
