use itertools::Itertools;
use native_db::*;
use native_model::{native_model, Model};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

use crate::{
    modules::{
        cache::{imap::mailbox::MailBox, vendor::outlook::model::MailFolder},
        database::{
            batch_delete_impl, batch_insert_impl, delete_impl, filter_by_secondary_key_impl,
            manager::DB_MANAGER, upsert_impl,
        },
        error::{code::ErrorCode, RustMailerError, RustMailerResult},
    },
    raise_error,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
#[native_model(id = 9, version = 1)]
#[native_db]
pub struct OutlookFolder {
    #[primary_key]
    pub id: u64,
    #[secondary_key]
    pub account_id: u64,
    pub name: String,
    pub exists: u32,
    pub unseen: Option<u32>,
    pub folder_id: String,
}

impl OutlookFolder {
    pub async fn upsert(folder: OutlookFolder) -> RustMailerResult<()> {
        upsert_impl(DB_MANAGER.envelope_db(), folder).await
    }

    pub async fn batch_insert(folders: &[OutlookFolder]) -> RustMailerResult<()> {
        batch_insert_impl(DB_MANAGER.envelope_db(), folders.to_vec()).await
    }

    pub async fn delete(id: u64) -> RustMailerResult<()> {
        delete_impl(DB_MANAGER.envelope_db(), move |rw| {
            rw.get()
                .primary::<OutlookFolder>(id)
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .ok_or_else(|| raise_error!("folder missing".into(), ErrorCode::InternalError))
        })
        .await
    }

    pub async fn list_all(account_id: u64) -> RustMailerResult<Vec<OutlookFolder>> {
        filter_by_secondary_key_impl(
            DB_MANAGER.envelope_db(),
            OutlookFolderKey::account_id,
            account_id,
        )
        .await
    }

    pub async fn get_by_name(account_id: u64, name: &str) -> RustMailerResult<Self> {
        let folders = Self::list_all(account_id).await?;
        folders
            .into_iter()
            .find(|folder| folder.name == name)
            .ok_or_else(|| {
                raise_error!(
                    format!("Folder '{}' not found for account {}", name, account_id),
                    ErrorCode::MailBoxNotCached
                )
            })
    }

    pub async fn batch_delete(folders: Vec<OutlookFolder>) -> RustMailerResult<()> {
        batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
            let mut to_deleted = Vec::new();
            for folder in folders {
                let retrived = rw
                    .get()
                    .primary::<OutlookFolder>(folder.id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
                if let Some(retrived) = retrived {
                    to_deleted.push(retrived);
                }
            }
            Ok(to_deleted)
        })
        .await?;
        Ok(())
    }

    pub async fn clean(account_id: u64) -> RustMailerResult<()> {
        batch_delete_impl(DB_MANAGER.envelope_db(), move |rw| {
            let folders: Vec<OutlookFolder> = rw
                .scan()
                .secondary::<OutlookFolder>(OutlookFolderKey::account_id)
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .start_with(account_id)
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                .try_collect()
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;
            Ok(folders)
        })
        .await?;
        Ok(())
    }
}

impl From<OutlookFolder> for MailBox {
    fn from(value: OutlookFolder) -> Self {
        Self {
            id: value.id,
            account_id: value.account_id,
            name: value.name,
            delimiter: Some("/".into()),
            attributes: vec![],
            flags: vec![],
            exists: value.exists,
            unseen: value.unseen,
            permanent_flags: vec![],
            uid_next: None,
            uid_validity: None,
            highest_modseq: None,
        }
    }
}

impl TryFrom<MailFolder> for OutlookFolder {
    type Error = RustMailerError;

    fn try_from(value: MailFolder) -> Result<Self, Self::Error> {
        Ok(Self {
            id: 0,
            account_id: 0,
            name: value.display_name,
            exists: value.total_item_count.ok_or_else(|| raise_error!("Graph API response missing totalItemCount — unexpected; this field should always be present".into(), ErrorCode::InternalError))?,
            unseen: value.unread_item_count,
            folder_id: value.id,
        })
    }
}
