#[cfg(not(test))]
use crate::modules::account::dispatcher::STATUS_DISPATCHER;
use crate::modules::account::entity::{Account, AuthType};
use crate::modules::error::code::ErrorCode;
use crate::modules::error::RustMailerResult;
#[cfg(not(test))]
use crate::modules::imap::capabilities::capability_to_string;
use crate::modules::imap::capabilities::{check_capabilities, fetch_capabilities};
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

    #[cfg(test)]
    pub async fn fetch_account(&self) -> RustMailerResult<Account> {
        // Return the default account in test environment
        Ok(default_account())
    }

    #[cfg(not(test))]
    pub async fn fetch_account(&self) -> RustMailerResult<Account> {
        // Fetch the account entity in non-test environment
        Account::get(self.account_id).await
    }

    async fn create_client(&self, account: &Account) -> RustMailerResult<Client> {
        Client::connection(
            account.imap.host.clone(),
            account.imap.encryption.clone(),
            account.imap.port,
            account.imap.use_proxy,
        )
        .await
    }

    async fn authenticate(
        &self,
        client: Client,
        account: &Account,
    ) -> RustMailerResult<Session<Box<dyn SessionStream>>> {
        match &account.imap.auth.auth_type {
            AuthType::Password => {
                let password = account.imap.auth.password.clone().ok_or_else(|| {
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
                #[cfg(not(test))]
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
                #[cfg(not(test))]
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
                #[cfg(not(test))]
                let to_save: Vec<String> = capabilities.iter().map(capability_to_string).collect();
                #[cfg(not(test))]
                Account::update_capabilities(self.account_id, to_save).await?;

                if let Err(error) = check_capabilities(&capabilities) {
                    error!("Failed to check IMAP capabilities: {:#?}", error);
                    #[cfg(not(test))]
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
                #[cfg(not(test))]
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

#[cfg(test)]
fn default_account() -> Account {
    use crate::modules::account::entity::{AuthConfig, Encryption, ImapConfig, SmtpConfig};

    let email = "test1@zohomail.com".to_string();
    let imap = ImapConfig {
        // Initialize with appropriate values
        //host: "imap.zoho.com".to_string(),
        host: "imap.zoho.com".to_string(),
        port: 993,
        encryption: Encryption::Ssl,
        auth: AuthConfig {
            auth_type: AuthType::Password,
            password: Some("xxxxxxxxxx".to_string()),
        },
        use_proxy: None,
    };
    let smtp = SmtpConfig {
        // Initialize with appropriate values
        host: "smtp.example.com".to_string(),
        port: 465,
        encryption: Encryption::Ssl,
        auth: AuthConfig {
            auth_type: AuthType::Password,
            password: Some("".to_string()),
        },
        use_proxy: None,
    };
    Account::new(email, None, imap, smtp, false, false, None, None, 100, 200)
}
