// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::account::dispatcher::STATUS_DISPATCHER;
use crate::modules::account::entity::AuthType;
use crate::modules::account::v2::AccountV2;
use crate::modules::error::code::ErrorCode;
use crate::modules::error::RustMailerResult;
use crate::modules::imap::capabilities::{
    capability_to_string, check_capabilities, fetch_capabilities,
};
use crate::modules::imap::client::Client;
use crate::modules::imap::oauth2::OAuth2;
use crate::modules::imap::session::SessionStream;
use crate::modules::oauth2::token::OAuth2AccessToken;
use crate::{decrypt, raise_error};
use async_imap::Session;
use tracing::error;

#[derive(Debug)]
pub struct ImapConnectionManager {
    pub account_id: u64,
}

impl ImapConnectionManager {
    pub fn new(account_id: u64) -> Self {
        Self { account_id }
    }

    pub async fn fetch_account(&self) -> RustMailerResult<AccountV2> {
        // Fetch the account entity in non-test environment
        AccountV2::get(self.account_id).await
    }

    async fn create_client(&self, account: &AccountV2) -> RustMailerResult<Client> {
        let imap = account
            .imap
            .clone()
            .expect("BUG: account.imap is None, but it should always be present");
        Client::connection(imap.host, imap.encryption, imap.port, imap.use_proxy).await
    }

    async fn authenticate(
        &self,
        client: Client,
        account: &AccountV2,
    ) -> RustMailerResult<Session<Box<dyn SessionStream>>> {
        let imap = account
            .imap
            .clone()
            .expect("BUG: account.imap is None, but it should always be present");

        match &imap.auth.auth_type {
            AuthType::Password => {
                let password = imap.auth.password.clone().ok_or_else(|| {
                    raise_error!(
                        "Imap auth type is Passwd, but password not set".into(),
                        ErrorCode::MissingConfiguration
                    )
                })?;

                let password = decrypt!(&password)?;
                client.login(&account.email, &password).await
            }
            AuthType::OAuth2 => {
                let record = OAuth2AccessToken::get(self.account_id).await?;
                let access_token = record.and_then(|r| r.access_token).ok_or_else(|| {
                    raise_error!(
                        "Imap auth type is OAuth2, but OAuth2 authorization is not yet complete."
                            .into(),
                        ErrorCode::MissingConfiguration
                    )
                })?;
                client
                    .authenticate(OAuth2::new(account.email.clone(), access_token))
                    .await
            }
        }
    }

    pub async fn build(&self) -> RustMailerResult<Session<Box<dyn SessionStream>>> {
        let account = self.fetch_account().await?;

        let client = match self.create_client(&account).await {
            Ok(client) => client,
            Err(error) => {
                error!(
                    "Failed to create IMAP {}'s client: {:#?}",
                    &account.email, error
                );
                STATUS_DISPATCHER
                    .append_error(
                        self.account_id,
                        format!("imap client connect error: {:#?}", error),
                    )
                    .await;
                return Err(error);
            }
        };

        let mut session = match self.authenticate(client, &account).await {
            Ok(session) => session,
            Err(error) => {
                error!("Failed to authenticate IMAP session: {:#?}", error);

                STATUS_DISPATCHER
                    .append_error(
                        self.account_id,
                        format!("imap client authenticate error: {:#?}", error),
                    )
                    .await;
                return Err(error);
            }
        };

        match fetch_capabilities(&mut session).await {
            Ok(capabilities) => {
                let to_save: Vec<String> = capabilities.iter().map(capability_to_string).collect();
                AccountV2::update_capabilities(self.account_id, to_save).await?;
                if let Err(error) = check_capabilities(&capabilities) {
                    error!("Failed to check IMAP capabilities: {:#?}", error);
                    STATUS_DISPATCHER
                        .append_error(
                            self.account_id,
                            format!("imap client check capabilities error: {:#?}", error),
                        )
                        .await;
                    return Err(error);
                }
            }
            Err(error) => {
                error!("Failed to fetch IMAP capabilities: {:#?}", error);
                STATUS_DISPATCHER
                    .append_error(
                        self.account_id,
                        format!("imap client fetch capabilities error: {:#?}", error),
                    )
                    .await;
                return Err(error);
            }
        }

        Ok(session)
    }
}
