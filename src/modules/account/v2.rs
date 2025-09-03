// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use native_db::*;
use native_model::{native_model, Model};

use poem_openapi::Object;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tracing::info;

use crate::{
    encrypt,
    modules::{
        account::{
            entity::{Account, ImapConfig, MailerType, SmtpConfig},
            since::DateSince,
            status::AccountRunningState,
        },
        cache::{
            imap::{
                address::AddressEntity, mailbox::MailBox, manager::FLAGS_STATE_MAP,
                minimal::MinimalEnvelope, thread::EmailThread, v2::EmailEnvelopeV3,
            },
            vendor::gmail::sync::{
                envelope::GmailEnvelope,
                labels::{GmailCheckPoint, GmailLabels},
            },
        },
        database::{insert_impl, list_all_impl},
        error::RustMailerResult,
    },
    utc_now,
};

use crate::id;
use crate::modules::account::payload::AccountCreateRequest;
use crate::modules::account::payload::AccountUpdateRequest;
use crate::modules::account::payload::MinimalAccount;
use crate::modules::cache::imap::task::SYNC_TASKS;
use crate::modules::context::controller::SYNC_CONTROLLER;
use crate::modules::context::executors::RUST_MAIL_CONTEXT;
use crate::modules::database::count_by_unique_secondary_key_impl;
use crate::modules::database::delete_impl;
use crate::modules::database::manager::DB_MANAGER;
use crate::modules::database::{
    paginate_query_primary_scan_all_impl, secondary_find_impl, update_impl,
};
use crate::modules::error::code::ErrorCode;
use crate::modules::hook::entity::EventHooks;
use crate::modules::license::License;
use crate::modules::oauth2::token::OAuth2AccessToken;
use crate::modules::rest::response::DataPage;
use crate::modules::smtp::template::entity::EmailTemplate;
use crate::modules::token::AccessToken;
use crate::raise_error;

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
#[native_model(id = 5, version = 2, from = Account)]
#[native_db(primary_key(pk -> String))]
pub struct AccountV2 {
    /// Unique account identifier
    #[secondary_key(unique)]
    pub id: u64,
    /// IMAP server configuration
    pub imap: Option<ImapConfig>,
    /// SMTP server configuration
    pub smtp: Option<SmtpConfig>,
    /// Represents the account activation status.
    ///
    /// If this value is `false`, all account-related resources will be unavailable
    /// and any attempts to access them should return an error indicating the account
    /// is inactive.
    pub enabled: bool,
    /// Method used to access and manage emails.
    pub mailer_type: MailerType,
    /// Email address associated with this account
    #[oai(validator(custom = "crate::modules::common::validator::EmailValidator"))]
    pub email: String,
    /// Display name for the account (optional)
    pub name: Option<String>,
    /// Minimal sync mode flag
    ///
    /// When enabled (`true`), only the most essential metadata will be synchronized:
    /// Recommended for:
    /// - Extremely resource-constrained environments
    /// - Accounts where only new message notification is needed
    pub minimal_sync: Option<bool>,
    /// IMAP Server-supported capability flags
    pub capabilities: Option<Vec<String>>,
    /// DSN (Delivery Status Notification) support flag
    pub dsn_capable: Option<bool>,
    /// Controls initial synchronization time range
    ///
    /// When dealing with large mailboxes, this restricts scanning to:
    /// - Messages after specified starting point
    /// - Or within sliding window
    ///
    /// ### Use Cases
    /// - Event-driven systems (only sync recent actionable emails)
    /// - First-time sync optimization for large accounts
    /// - Reducing server load during resyncs
    pub date_since: Option<DateSince>,
    /// Configuration for selective folder synchronization
    ///
    /// Defaults to standard folders (`INBOX`, `Sent`) if empty.
    /// Modified folders will be automatically synced on next update.
    pub sync_folders: Vec<String>,
    /// Full sync interval (minutes), default 30m
    pub full_sync_interval_min: Option<i64>,
    /// Incremental sync interval (seconds), default 60s
    pub incremental_sync_interval_sec: i64,
    /// Tracks known mail folders and detects changes (creations/deletions)
    pub known_folders: BTreeSet<String>,
    /// Creation timestamp (UNIX epoch milliseconds)
    pub created_at: i64,
    /// Last update timestamp (UNIX epoch milliseconds)
    pub updated_at: i64,
    /// Optional proxy ID for establishing the connection to external APIs (e.g., Gmail, Outlook).
    /// - If `None` or not provided, the client will connect directly to the API server.
    /// - If `Some(proxy_id)`, the client will use the pre-configured proxy with the given ID for API requests.
    pub use_proxy: Option<u64>,
}

impl AccountV2 {
    fn pk(&self) -> String {
        format!("{}_{}", self.created_at, self.id)
    }

    pub fn minimal_sync(&self) -> bool {
        self.minimal_sync.unwrap_or(false)
    }

    pub fn create(request: AccountCreateRequest) -> RustMailerResult<Self> {
        Ok(Self {
            id: id!(64),
            email: request.email,
            name: request.name,
            imap: request
                .imap
                .map(|imap| imap.try_encrypt_password())
                .transpose()?,
            smtp: request
                .smtp
                .map(|smtp| smtp.try_encrypt_password())
                .transpose()?,
            enabled: request.enabled,
            mailer_type: request.mailer_type,
            minimal_sync: request.minimal_sync,
            capabilities: None,
            date_since: request.date_since,
            dsn_capable: None,
            sync_folders: vec![],
            known_folders: BTreeSet::new(),
            full_sync_interval_min: request.full_sync_interval_min,
            incremental_sync_interval_sec: request.incremental_sync_interval_sec,
            created_at: utc_now!(),
            updated_at: utc_now!(),
            use_proxy: request.use_proxy,
        })
    }

    pub async fn check_account_active(
        account_id: u64,
        imap_only: bool,
    ) -> RustMailerResult<AccountV2> {
        let account =
            secondary_find_impl::<AccountV2>(DB_MANAGER.meta_db(), AccountV2Key::id, account_id)
                .await?
                .ok_or_else(|| {
                    raise_error!(
                        format!("Account id='{account_id}' not found"),
                        ErrorCode::ResourceNotFound
                    )
                })?;

        if !account.enabled {
            return Err(raise_error!(
                format!("Account id='{account_id}' is disabled"),
                ErrorCode::AccountDisabled
            ));
        }

        if imap_only && !matches!(account.mailer_type, MailerType::ImapSmtp) {
            return Err(raise_error!(
                format!("Account id='{account_id}' is not IMAP/SMTP compatible"),
                ErrorCode::Incompatible
            ));
        }

        Ok(account)
    }

    /// Fetches an `AccountEntity` by its `id`.
    pub async fn get(account_id: u64) -> RustMailerResult<AccountV2> {
        let result = Self::find(account_id).await?.ok_or_else(|| {
            raise_error!(
                format!("Account with ID '{account_id}' not found"),
                ErrorCode::ResourceNotFound
            )
        })?;
        Ok(result)
    }

    pub async fn find(account_id: u64) -> RustMailerResult<Option<AccountV2>> {
        secondary_find_impl::<AccountV2>(DB_MANAGER.meta_db(), AccountV2Key::id, account_id).await
    }

    /// Saves the current `AccountEntity` by persisting it to storage.
    pub async fn save(&self) -> RustMailerResult<()> {
        insert_impl(DB_MANAGER.meta_db(), self.to_owned()).await
    }

    pub async fn create_account(request: AccountCreateRequest) -> RustMailerResult<AccountV2> {
        // Validate license limits before creating entity
        if let Some(license) = License::get_current_license().await? {
            let current_count = AccountV2::count().await?;
            if let Some(max_accounts) = license.max_accounts {
                if current_count >= max_accounts as usize {
                    return Err(raise_error!(
                        "Maximum account limit reached".into(),
                        ErrorCode::LicenseAccountLimitReached
                    ));
                }
            }
        }
        let entity = request.create_entity()?;
        entity.save().await?;
        SYNC_CONTROLLER
            .trigger_start(entity.id, entity.email.clone())
            .await;
        Ok(entity)
    }

    pub async fn update(
        account_id: u64,
        request: AccountUpdateRequest,
        validate: bool,
    ) -> RustMailerResult<()> {
        if validate {
            request.validate_update_request()?;
        }
        update_impl(DB_MANAGER.meta_db(), move |rw| {
            rw.get().secondary::<AccountV2>(AccountV2Key::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?.ok_or_else(|| raise_error!(format!(
                "Attempted to edit the account's base information, but the corresponding account metadata was not found. account_id={}",
                account_id
            ), ErrorCode::ResourceNotFound))
        }, |current|{
            Self::apply_update_fields(current, request)
        }).await?;

        Ok(())
    }

    pub async fn delete(account_id: u64) -> RustMailerResult<()> {
        let request = AccountUpdateRequest {
            enabled: Some(false),
            ..Default::default()
        };
        Self::update(account_id, request, false).await?;
        SYNC_TASKS.stop(account_id).await?;
        Self::cleanup_account_resources_sequential(account_id).await
    }

    async fn delete_account(account_id: u64) -> RustMailerResult<()> {
        delete_impl(DB_MANAGER.meta_db(), move|rw|{
            rw.get().secondary::<AccountV2>(AccountV2Key::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(||raise_error!(format!("The account entity with id={account_id} that you want to delete was not found."), ErrorCode::ResourceNotFound))
        }).await
    }

    async fn cleanup_account_resources_sequential(account_id: u64) -> RustMailerResult<()> {
        let account = Self::get(account_id).await?;
        EmailTemplate::remove_account_templates(account_id).await?;
        OAuth2AccessToken::try_delete(account_id).await?;
        EventHooks::try_delete(account_id).await?;
        AccessToken::cleanup_account(account_id).await?;
        AccountRunningState::delete(account_id).await?;
        match account.mailer_type {
            MailerType::ImapSmtp => {
                MailBox::clean(account_id).await?;
                FLAGS_STATE_MAP.remove(&account.id);
                EmailEnvelopeV3::clean_account(account.id).await?;
                MinimalEnvelope::clean_account(account.id).await?;
                RUST_MAIL_CONTEXT.clean_account(account_id).await?;
            }
            MailerType::GmailApi => {
                GmailLabels::clean(account_id).await?;
                GmailEnvelope::clean_account(account.id).await?;
                GmailCheckPoint::clean(account.id).await?;
            }
        }
        AddressEntity::clean_account(account.id).await?;
        EmailThread::clean_account(account.id).await?;
        Self::delete_account(account_id).await?;
        info!("Sequential cleanup completed for account: {}", account_id);
        Ok(())
    }

    pub async fn update_sync_folders(
        account_id: u64,
        sync_folders: Vec<String>,
    ) -> RustMailerResult<()> {
        update_impl(DB_MANAGER.meta_db(), move |rw| {
            rw.get().secondary::<AccountV2>(AccountV2Key::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!("When trying to update account sync_folders, the corresponding record was not found. account_id={}", account_id), ErrorCode::ResourceNotFound))
        }, |current|{
            let mut updated = current.clone();
            updated.sync_folders = sync_folders;
            Ok(updated)
        }).await?;
        Ok(())
    }

    pub async fn update_known_folders(
        account_id: u64,
        known_folders: BTreeSet<String>,
    ) -> RustMailerResult<()> {
        update_impl(DB_MANAGER.meta_db(), move |rw| {
            rw.get().secondary::<AccountV2>(AccountV2Key::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!("When trying to update account known_folders, the corresponding record was not found. account_id={}", account_id), ErrorCode::ResourceNotFound))
        }, |current|{
            let mut updated = current.clone();
            updated.known_folders = known_folders;
            Ok(updated)
        }).await?;
        Ok(())
    }

    pub async fn update_capabilities(
        account_id: u64,
        capabilities: Vec<String>,
    ) -> RustMailerResult<()> {
        update_impl(DB_MANAGER.meta_db(), move |rw| {
            rw.get().secondary::<AccountV2>(AccountV2Key::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!("When trying to update account capabilities, the corresponding record was not found. account_id={}", account_id), ErrorCode::ResourceNotFound))
        }, |current|{
            let mut updated = current.clone();
            updated.capabilities = Some(capabilities);
            Ok(updated)
        }).await?;
        Ok(())
    }

    pub async fn update_dsn_capable(account_id: u64, dsn: bool) -> RustMailerResult<()> {
        update_impl(
            DB_MANAGER.meta_db(),
            move |rw| {
                rw.get()
                    .secondary::<AccountV2>(AccountV2Key::id, account_id)
                    .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
                    .ok_or_else(|| {
                        raise_error!(format!(
                            "When trying to update account dsn capabilities, the corresponding record was not found. account_id={}",
                            account_id
                        ), ErrorCode::ResourceNotFound)
                    })
            },
            move |current| {
                let mut updated = current.clone();
                updated.dsn_capable = Some(dsn);
                Ok(updated)
            },
        )
        .await?;
        Ok(())
    }

    /// Retrieves a list of all `AccountEntity` instances.
    pub async fn list_all() -> RustMailerResult<Vec<AccountV2>> {
        list_all_impl(DB_MANAGER.meta_db()).await
    }

    pub async fn minimal_list() -> RustMailerResult<Vec<MinimalAccount>> {
        let result = list_all_impl(DB_MANAGER.meta_db())
            .await?
            .into_iter()
            .filter(|a: &AccountV2| a.enabled)
            .map(|account: AccountV2| MinimalAccount {
                id: account.id,
                email: account.email.clone(),
            })
            .collect::<Vec<MinimalAccount>>();
        Ok(result)
    }

    pub async fn count() -> RustMailerResult<usize> {
        count_by_unique_secondary_key_impl::<AccountV2>(DB_MANAGER.meta_db(), AccountV2Key::id)
            .await
    }

    pub async fn paginate_list(
        page: Option<u64>,
        page_size: Option<u64>,
        desc: Option<bool>,
    ) -> RustMailerResult<DataPage<AccountV2>> {
        paginate_query_primary_scan_all_impl(DB_MANAGER.meta_db(), page, page_size, desc)
            .await
            .map(DataPage::from)
    }

    // This method applies the updates from the request to the old account entity
    fn apply_update_fields(
        old: &AccountV2,
        request: AccountUpdateRequest,
    ) -> RustMailerResult<AccountV2> {
        let mut new = old.clone();

        if let Some(date_since) = request.date_since {
            new.date_since = Some(date_since);
        }

        if let Some(name) = &request.name {
            new.name = Some(name.clone());
        }

        if let Some(imap) = &request.imap {
            if let Some(current_imap) = &mut new.imap {
                current_imap.host = imap.host.clone();
                current_imap.port = imap.port.clone();
                current_imap.encryption = imap.encryption.clone();
                current_imap.auth.auth_type = imap.auth.auth_type.clone();
                if let Some(password) = &imap.auth.password {
                    let encrypted_password = encrypt!(password)?;
                    current_imap.auth.password = Some(encrypted_password);
                }
                current_imap.use_proxy = imap.use_proxy;
            }
        }

        if let Some(smtp) = &request.smtp {
            if let Some(current_smtp) = &mut new.smtp {
                current_smtp.host = smtp.host.clone();
                current_smtp.port = smtp.port.clone();
                current_smtp.encryption = smtp.encryption.clone();
                current_smtp.auth.auth_type = smtp.auth.auth_type.clone();
                if let Some(password) = &smtp.auth.password {
                    let encrypted_password = encrypt!(password)?;
                    current_smtp.auth.password = Some(encrypted_password);
                }
                current_smtp.use_proxy = smtp.use_proxy;
            }
        }

        if let Some(mailboxes) = request.sync_folders {
            new.sync_folders = mailboxes;
        }

        if let Some(use_proxy) = request.use_proxy {
            new.use_proxy = Some(use_proxy);
        }

        if let Some(full_sync_interval_min) = &request.full_sync_interval_min {
            new.full_sync_interval_min = Some(*full_sync_interval_min);
        }
        if let Some(incremental_sync_interval_sec) = &request.incremental_sync_interval_sec {
            new.incremental_sync_interval_sec = *incremental_sync_interval_sec;
        }
        if let Some(enabled) = request.enabled {
            new.enabled = enabled;
        }
        new.updated_at = utc_now!();
        Ok(new)
    }
}

// Will never be used
impl From<AccountV2> for Account {
    fn from(value: AccountV2) -> Self {
        Self {
            id: value.id,
            imap: value.imap.unwrap(),
            smtp: value.smtp.unwrap(),
            enabled: value.enabled,
            email: value.email,
            name: value.name,
            minimal_sync: value.minimal_sync.unwrap(),
            capabilities: value.capabilities.unwrap(),
            dsn_capable: value.dsn_capable,
            date_since: value.date_since,
            sync_folders: value.sync_folders,
            full_sync_interval_min: value.full_sync_interval_min.unwrap(),
            incremental_sync_interval_sec: value.incremental_sync_interval_sec,
            known_folders: value.known_folders,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

impl From<Account> for AccountV2 {
    fn from(value: Account) -> Self {
        Self {
            id: value.id,
            imap: Some(value.imap),
            smtp: Some(value.smtp),
            enabled: value.enabled,
            mailer_type: MailerType::ImapSmtp,
            email: value.email,
            name: value.name,
            minimal_sync: Some(value.minimal_sync),
            capabilities: Some(value.capabilities),
            dsn_capable: value.dsn_capable,
            date_since: value.date_since,
            sync_folders: value.sync_folders,
            full_sync_interval_min: Some(value.full_sync_interval_min),
            incremental_sync_interval_sec: value.incremental_sync_interval_sec,
            known_folders: value.known_folders,
            created_at: value.created_at,
            updated_at: value.updated_at,
            use_proxy: None,
        }
    }
}
