// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::collections::BTreeSet;

use crate::encrypt;
use crate::id;
use crate::modules::account::payload::AccountCreateRequest;
use crate::modules::account::payload::AccountUpdateRequest;
use crate::modules::account::payload::MinimalAccount;
use crate::modules::account::since::DateSince;
use crate::modules::cache::imap::mailbox::MailBox;
use crate::modules::cache::imap::manager::EnvelopeFlagsManager;
use crate::modules::cache::imap::task::IMAP_TASKS;
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
use crate::{
    modules::database::{insert_impl, list_all_impl},
    modules::error::RustMailerResult,
    utc_now,
};
use native_db::*;
use native_model::{native_model, Model};

use poem_openapi::{Enum, Object};
use serde::{Deserialize, Serialize};
use tracing::error;
use tracing::info;

use super::status::AccountRunningState;

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
#[native_model(id = 5, version = 1)]
#[native_db(primary_key(pk -> String))]
pub struct Account {
    /// Unique account identifier
    #[secondary_key(unique)]
    pub id: u64,
    /// IMAP server configuration
    pub imap: ImapConfig,
    /// SMTP server configuration
    pub smtp: SmtpConfig,
    /// Represents the account activation status.
    ///
    /// If this value is `false`, all account-related resources will be unavailable
    /// and any attempts to access them should return an error indicating the account
    /// is inactive.
    pub enabled: bool,
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
    pub minimal_sync: bool,
    /// IMAP Server-supported capability flags
    pub capabilities: Vec<String>,
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
    pub full_sync_interval_min: i64,
    /// Incremental sync interval (seconds), default 60s
    pub incremental_sync_interval_sec: i64,
    /// Tracks known mail folders and detects changes (creations/deletions)
    pub known_folders: BTreeSet<String>,
    /// Creation timestamp (UNIX epoch milliseconds)
    pub created_at: i64,
    /// Last update timestamp (UNIX epoch milliseconds)
    pub updated_at: i64,
}

impl Account {
    fn pk(&self) -> String {
        format!("{}_{}", self.created_at, self.id)
    }

    pub fn create(request: AccountCreateRequest) -> RustMailerResult<Self> {
        Ok(Account {
            id: id!(64),
            email: request.email,
            name: request.name,
            imap: request.imap.try_encrypt_password()?,
            smtp: request.smtp.try_encrypt_password()?,
            enabled: request.enabled,
            minimal_sync: request.minimal_sync,
            capabilities: Vec::new(),
            //status: AccountStatus::Registered,
            //error_reason: None,
            date_since: request.date_since,
            dsn_capable: None,
            sync_folders: vec![],
            known_folders: BTreeSet::new(),
            full_sync_interval_min: request.full_sync_interval_min.unwrap_or(30),
            incremental_sync_interval_sec: request.incremental_sync_interval_sec.unwrap_or(60),
            created_at: utc_now!(),
            updated_at: utc_now!(),
        })
    }

    pub async fn check_account_active(account_id: u64) -> RustMailerResult<Account> {
        let account_entity =
            secondary_find_impl::<Account>(DB_MANAGER.meta_db(), AccountKey::id, account_id)
                .await?;
        match account_entity {
            Some(entity) if entity.enabled => Ok(entity),
            Some(_) => Err(raise_error!(
                format!("Account id='{account_id}' is disabled"),
                ErrorCode::AccountDisabled
            )),
            None => Err(raise_error!(
                format!("Account id='{account_id}' not found"),
                ErrorCode::ResourceNotFound
            )),
        }
    }

    /// Fetches an `AccountEntity` by its `id`.
    pub async fn get(account_id: u64) -> RustMailerResult<Account> {
        let result = Self::find(account_id).await?.ok_or_else(|| {
            raise_error!(
                format!("Account with ID '{account_id}' not found"),
                ErrorCode::ResourceNotFound
            )
        })?;
        Ok(result)
    }

    pub async fn find(account_id: u64) -> RustMailerResult<Option<Account>> {
        secondary_find_impl::<Account>(DB_MANAGER.meta_db(), AccountKey::id, account_id).await
    }

    /// Saves the current `AccountEntity` by persisting it to storage.
    pub async fn save(&self) -> RustMailerResult<()> {
        insert_impl(DB_MANAGER.meta_db(), self.to_owned()).await
    }

    pub async fn create_account(request: AccountCreateRequest) -> RustMailerResult<Account> {
        // Validate license limits before creating entity
        if let Some(license) = License::get_current_license().await? {
            let current_count = Account::count().await?;
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
            rw.get().secondary::<Account>(AccountKey::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?.ok_or_else(|| raise_error!(format!(
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
        IMAP_TASKS.stop(account_id).await?;
        tokio::spawn(async move {
            if let Err(e) = Self::cleanup_account_resources_sequential(account_id).await {
                error!("Account cleanup failed for {}: {:?}", account_id, e);
            }
        });
        Ok(())
    }

    async fn delete_account(account_id: u64) -> RustMailerResult<()> {
        delete_impl(DB_MANAGER.meta_db(), move|rw|{
            rw.get().secondary::<Account>(AccountKey::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(||raise_error!(format!("The account entity with id={account_id} that you want to delete was not found."), ErrorCode::ResourceNotFound))
        }).await
    }

    async fn cleanup_account_resources_sequential(account_id: u64) -> RustMailerResult<()> {
        let account = Self::get(account_id).await?;
        EmailTemplate::remove_account_templates(account_id).await?;
        OAuth2AccessToken::try_delete(account_id).await?;
        EventHooks::try_delete(account_id).await?;
        AccessToken::cleanup_account(account_id).await?;
        MailBox::clean(account_id).await?;
        AccountRunningState::delete(account_id).await?;
        //INDEX_MANAGER.clean(&account).await?;
        EnvelopeFlagsManager::clean_account(account.id).await?;
        RUST_MAIL_CONTEXT.clean_account(account_id).await?;
        Self::delete_account(account_id).await?;
        info!("Sequential cleanup completed for account: {}", account_id);
        Ok(())
    }

    pub async fn update_sync_folders(
        account_id: u64,
        sync_folders: Vec<String>,
    ) -> RustMailerResult<()> {
        update_impl(DB_MANAGER.meta_db(), move |rw| {
            rw.get().secondary::<Account>(AccountKey::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
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
            rw.get().secondary::<Account>(AccountKey::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!("When trying to update account known_folders, the corresponding record was not found. account_id={}", account_id), ErrorCode::ResourceNotFound))
        }, |current|{
            let mut updated = current.clone();
            updated.known_folders = known_folders;
            Ok(updated)
        }).await?;
        Ok(())
    }

    #[cfg(not(test))]
    pub async fn update_capabilities(
        account_id: u64,
        capabilities: Vec<String>,
    ) -> RustMailerResult<()> {
        update_impl(DB_MANAGER.meta_db(), move |rw| {
            rw.get().secondary::<Account>(AccountKey::id, account_id).map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!("When trying to update account capabilities, the corresponding record was not found. account_id={}", account_id), ErrorCode::ResourceNotFound))
        }, |current|{
            let mut updated = current.clone();
            updated.capabilities = capabilities;
            Ok(updated)
        }).await?;
        Ok(())
    }

    pub async fn update_dsn_capable(account_id: u64, dsn: bool) -> RustMailerResult<()> {
        update_impl(
            DB_MANAGER.meta_db(),
            move |rw| {
                rw.get()
                    .secondary::<Account>(AccountKey::id, account_id)
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
    pub async fn list_all() -> RustMailerResult<Vec<Account>> {
        list_all_impl(DB_MANAGER.meta_db()).await
    }

    pub async fn minimal_list() -> RustMailerResult<Vec<MinimalAccount>> {
        let result = list_all_impl(DB_MANAGER.meta_db())
            .await?
            .into_iter()
            .filter(|a: &Account| a.enabled)
            .map(|account: Account| MinimalAccount {
                id: account.id,
                email: account.email.clone(),
            })
            .collect::<Vec<MinimalAccount>>();
        Ok(result)
    }

    pub async fn count() -> RustMailerResult<usize> {
        count_by_unique_secondary_key_impl::<Account>(DB_MANAGER.meta_db(), AccountKey::id).await
    }

    pub async fn paginate_list(
        page: Option<u64>,
        page_size: Option<u64>,
        desc: Option<bool>,
    ) -> RustMailerResult<DataPage<Account>> {
        paginate_query_primary_scan_all_impl(DB_MANAGER.meta_db(), page, page_size, desc)
            .await
            .map(DataPage::from)
    }

    // This method applies the updates from the request to the old account entity
    fn apply_update_fields(
        old: &Account,
        request: AccountUpdateRequest,
    ) -> RustMailerResult<Account> {
        let mut new = old.clone();

        if let Some(date_since) = request.date_since {
            new.date_since = Some(date_since);
        }

        if let Some(name) = &request.name {
            new.name = Some(name.clone());
        }

        if let Some(imap) = &request.imap {
            new.imap.host = imap.host.clone();
            new.imap.port = imap.port.clone();
            new.imap.encryption = imap.encryption.clone();
            new.imap.auth.auth_type = imap.auth.auth_type.clone();
            if let Some(password) = &imap.auth.password {
                let encrypted_password = encrypt!(password)?;
                new.imap.auth.password = Some(encrypted_password);
            }
            new.imap.use_proxy = imap.use_proxy;
        }

        if let Some(smtp) = &request.smtp {
            new.smtp.host = smtp.host.clone();
            new.smtp.port = smtp.port.clone();
            new.smtp.encryption = smtp.encryption.clone();
            new.smtp.auth.auth_type = smtp.auth.auth_type.clone();
            if let Some(password) = &smtp.auth.password {
                let encrypted_password = encrypt!(password)?;
                new.smtp.auth.password = Some(encrypted_password);
            }
            new.smtp.use_proxy = smtp.use_proxy;
        }

        if let Some(mailboxes) = request.sync_folders {
            new.sync_folders = mailboxes;
        }
        if let Some(full_sync_interval_min) = &request.full_sync_interval_min {
            new.full_sync_interval_min = *full_sync_interval_min;
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

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize, Object)]
pub struct ImapConfig {
    /// IMAP server hostname or IP address
    #[oai(validator(max_length = 253, pattern = r"^[a-zA-Z0-9\-\.]+$"))]
    pub host: String,
    /// IMAP server port number
    #[oai(validator(minimum(value = "1"), maximum(value = "65535")))]
    pub port: u16,
    /// Connection encryption method
    pub encryption: Encryption,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Optional proxy ID for establishing the connection.
    /// - If `None` or not provided, the client will connect directly to the IMAP server.
    /// - If `Some(proxy_id)`, the client will use the pre-configured proxy with the given ID.
    pub use_proxy: Option<u64>,
}

impl ImapConfig {
    pub fn try_encrypt_password(self) -> RustMailerResult<Self> {
        Ok(Self {
            host: self.host,
            port: self.port,
            encryption: self.encryption,
            auth: self.auth.encrypt()?,
            use_proxy: self.use_proxy,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Object)]
pub struct SmtpConfig {
    /// SMTP server hostname or IP address
    #[oai(validator(max_length = 253, pattern = r"^[a-zA-Z0-9\-\.]+$"))]
    pub host: String,
    /// SMTP server port number
    #[oai(validator(minimum(value = "1"), maximum(value = "65535")))]
    pub port: u16,
    /// Connection encryption method
    pub encryption: Encryption,
    /// Authentication configuration
    pub auth: AuthConfig,
    /// Optional proxy ID for establishing the connection.
    /// - If `None` or not provided, the client will connect directly to the SMTP server.
    /// - If `Some(proxy_id)`, the client will use the pre-configured proxy with the given ID.
    pub use_proxy: Option<u64>,
}

impl SmtpConfig {
    pub fn try_encrypt_password(self) -> RustMailerResult<Self> {
        Ok(Self {
            host: self.host,
            port: self.port,
            encryption: self.encryption,
            auth: self.auth.encrypt()?,
            use_proxy: self.use_proxy,
        })
    }
}

#[derive(Enum, Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum AuthType {
    /// Standard password authentication (PLAIN/LOGIN)
    #[default]
    Password,
    /// OAuth 2.0 authentication (SASL XOAUTH2)
    OAuth2,
}

#[derive(Object, Default, Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AuthConfig {
    ///Authentication method to use
    pub auth_type: AuthType,
    /// Credential secret for Password authentication.
    ///
    /// Users should provide a plaintext password (1 to 256 characters).
    /// The server will encrypt the password using AES-256-GCM and securely store it.
    /// The plaintext password is never stored, so users must remember it for authentication.
    #[oai(validator(max_length = 256, min_length = 1))]
    pub password: Option<String>,
}

impl AuthConfig {
    pub fn encrypt(self) -> RustMailerResult<Self> {
        match self.password {
            Some(password) => Ok(Self {
                auth_type: self.auth_type,
                password: Some(encrypt!(&password)?),
            }),
            None => Ok(self),
        }
    }
}

impl AuthConfig {
    pub fn validate(&self) -> Result<(), &'static str> {
        match self.auth_type {
            AuthType::Password if self.password.is_none() => {
                Err("When auth_type is Passwd, password must not be None.")
            }
            _ => Ok(()),
        }
    }
}

#[derive(Clone, Default, Debug, Eq, PartialEq, Serialize, Deserialize, Enum)]
pub enum Encryption {
    /// SSL/TLS encrypted connection
    #[default]
    Ssl,
    /// StartTLS encryption
    StartTls,
    /// Unencrypted connection
    None,
}

impl From<bool> for Encryption {
    fn from(value: bool) -> Self {
        if value {
            Self::Ssl
        } else {
            Self::None
        }
    }
}
