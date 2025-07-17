use std::collections::BTreeSet;

use crate::modules::account::entity::{Account, ImapConfig, SmtpConfig};
use crate::modules::account::since::DateSince;
use crate::modules::error::code::ErrorCode;
use crate::modules::error::RustMailerResult;
use crate::modules::token::AccountInfo;
use crate::{raise_error, validate_email};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct AccountCreateRequest {
    /// Email address associated with this account
    ///
    /// This field represents the user's email address and is required for account creation.
    /// Once set during account creation, the email address cannot be modified.
    #[oai(validator(custom = "crate::modules::common::validator::EmailValidator"))]
    pub email: String,
    /// Display name for the account (optional)
    pub name: Option<String>,
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
    /// Minimal sync mode flag
    ///
    /// When enabled (`true`), only the most essential metadata will be synchronized:
    /// Recommended for:
    /// - Extremely resource-constrained environments
    /// - Accounts where only new message notification is needed
    pub minimal_sync: bool,
    /// Full sync interval (minutes), default 30m
    #[oai(validator(minimum(value = "10"), maximum(value = "10080")))]
    pub full_sync_interval_min: Option<i64>,
    /// Incremental sync interval (seconds), default 60s
    #[oai(validator(minimum(value = "10"), maximum(value = "3600")))]
    pub incremental_sync_interval_sec: Option<i64>,
}

impl AccountCreateRequest {
    pub fn create_entity(self) -> RustMailerResult<Account> {
        if let Some(date_since) = self.date_since.as_ref() {
            date_since.validate()?;
        }

        Self::validate_request(&self.imap, &self.smtp, &self.email)?;
        Ok(Account::create(self)?)
    }

    fn validate_request(imap: &ImapConfig, smtp: &SmtpConfig, email: &str) -> RustMailerResult<()> {
        imap.auth
            .validate()
            .map_err(|e| raise_error!(e.to_owned(), ErrorCode::InvalidParameter))?;
        smtp.auth
            .validate()
            .map_err(|e| raise_error!(e.to_owned(), ErrorCode::InvalidParameter))?;

        validate_email!(email)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct AccountUpdateRequest {
    /// Represents the account activation status.
    ///
    /// If this value is `false`, all account-related resources will be unavailable
    /// and any attempts to access them should return an error indicating the account
    /// is inactive.
    pub enabled: Option<bool>,
    /// Display name for the account (optional)
    pub name: Option<String>,
    /// IMAP server configuration
    pub imap: Option<ImapConfig>,
    /// SMTP server configuration
    pub smtp: Option<SmtpConfig>,
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
    pub sync_folders: Option<Vec<String>>,
    /// Full sync interval (minutes)
    #[oai(validator(minimum(value = "10"), maximum(value = "10080")))]
    pub full_sync_interval_min: Option<i64>,
    /// Incremental sync interval (seconds)
    #[oai(validator(minimum(value = "10"), maximum(value = "3600")))]
    pub incremental_sync_interval_sec: Option<i64>,
}

impl AccountUpdateRequest {
    pub fn validate_update_request(&self) -> RustMailerResult<()> {
        if let Some(date_since) = self.date_since.as_ref() {
            date_since.validate()?;
        }

        if let Some(mailboxes) = self.sync_folders.as_ref() {
            if mailboxes.is_empty() {
                return Err(raise_error!(
                    "Invalid configuration: 'sync_folders' cannot be empty. \
                     If you are modifying the subscription list, please provide at least one mailbox to subscribe to.".into(), ErrorCode::InvalidParameter
                ));
            }
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]

pub struct MinimalAccount {
    pub id: u64,
    pub email: String,
}

pub fn filter_accessible_accounts<'a>(
    all_accounts: &'a [MinimalAccount],
    allowed: &BTreeSet<AccountInfo>,
) -> Vec<MinimalAccount> {
    all_accounts
        .iter()
        .filter(|acct| allowed.iter().any(|a| a.id == acct.id))
        .cloned()
        .collect()
}
