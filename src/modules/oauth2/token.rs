use crate::{
    decrypt, encrypt,
    modules::{
        database::{
            async_find_impl, delete_impl, list_all_impl, manager::DB_MANAGER, update_impl,
            upsert_impl,
        },
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error, utc_now,
};
use native_db::*;
use native_model::{native_model, Model};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, Object)]
#[native_model(id = 10, version = 1)]
#[native_db]
pub struct OAuth2AccessToken {
    /// The ID of the account associated with this access token.
    #[primary_key]
    pub account_id: u64,
    /// The name of the OAuth2 configuration associated with this access token.
    #[secondary_key]
    pub oauth2_id: u64,
    /// The OAuth2 access token used to authenticate requests to the provider.
    pub access_token: Option<String>,
    /// The OAuth2 refresh token used to obtain new access tokens.
    pub refresh_token: Option<String>,
    /// The timestamp when the token record was created, in milliseconds since the Unix epoch.
    pub created_at: i64,
    /// The timestamp when the token record was last updated, in milliseconds since the Unix epoch.
    pub updated_at: i64,
}

impl OAuth2AccessToken {
    pub fn create(
        account_id: u64,
        oauth2_id: u64,
        access_token: String,
        refresh_token: String,
    ) -> RustMailerResult<Self> {
        Ok(Self {
            account_id,
            oauth2_id,
            access_token: Some(encrypt!(&access_token)?),
            refresh_token: Some(encrypt!(&refresh_token)?),
            created_at: utc_now!(),
            updated_at: utc_now!(),
        })
    }

    // This function may be called multiple times for one account, so we use upsert.
    pub async fn save_or_update(&self) -> RustMailerResult<()> {
        upsert_impl(DB_MANAGER.meta_db(), self.clone()).await
    }

    pub async fn get(account_id: u64) -> RustMailerResult<Option<OAuth2AccessToken>> {
        async_find_impl::<OAuth2AccessToken>(DB_MANAGER.meta_db(), account_id)
            .await?
            .map(|mut token| {
                token.access_token = token.access_token.map(|t| decrypt!(&t)).transpose()?;
                token.refresh_token = token.refresh_token.map(|t| decrypt!(&t)).transpose()?;
                Ok(token)
            })
            .transpose()
    }

    pub async fn list_all() -> RustMailerResult<Vec<OAuth2AccessToken>> {
        list_all_impl::<OAuth2AccessToken>(DB_MANAGER.meta_db())
            .await?
            .into_iter()
            .map(|mut token| {
                token.access_token = token.access_token.map(|t| decrypt!(&t)).transpose()?;
                token.refresh_token = token.refresh_token.map(|t| decrypt!(&t)).transpose()?;
                Ok(token)
            })
            .collect()
    }

    pub async fn try_delete(account_id: u64) -> RustMailerResult<()> {
        if Self::get(account_id).await?.is_none() {
            return Ok(());
        }

        delete_impl(DB_MANAGER.meta_db(), move |rw|{
            rw.get().primary::<OAuth2AccessToken>(account_id)
            .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!(
                "The oauth2 access token entity with account_id={account_id} that you want to delete was not found."
            ),ErrorCode::ResourceNotFound))
        }).await
    }

    pub async fn delete_by_oauth2_id(oauth2_id: u64) -> RustMailerResult<()> {
        delete_impl(DB_MANAGER.meta_db(), move |rw|{
            rw.get().secondary::<OAuth2AccessToken>(OAuth2AccessTokenKey::oauth2_id, oauth2_id)
            .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!(
                "The oauth2 access token entity with oauth2_id={oauth2_id} that you want to delete was not found."
            ),ErrorCode::ResourceNotFound))
        }).await
    }

    pub async fn set_access_token(
        account_id: u64,
        access_token: String,
        refresh_token: String,
    ) -> RustMailerResult<()> {
        update_impl(DB_MANAGER.meta_db(), move |rw|{
            rw.get().primary::<OAuth2AccessToken>(account_id)
            .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?
            .ok_or_else(|| raise_error!(format!(
                "The oauth2 access token entity with account_id={account_id} that you want to modify was not found."
            ),ErrorCode::ResourceNotFound))
        }, |current| {
            let mut updated = current.clone();
            updated.access_token = Some(access_token);
            updated.refresh_token = Some(refresh_token);
            updated.updated_at = utc_now!();
            Ok(updated)
        }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::modules::oauth2::token::OAuth2AccessToken;

    #[tokio::test]
    async fn test1() {
        let token = OAuth2AccessToken::create(
            1000u64,
            1020u64,
            "access_token".into(),
            "refresh_token".into(),
        )
        .unwrap();
        token.save_or_update().await.unwrap();
        let token2 = OAuth2AccessToken::get(1000u64).await.unwrap().unwrap();
        assert_eq!(token2.access_token, Some("access_token".into()));
        assert_eq!(token2.refresh_token, Some("refresh_token".into()));

        let tokens = OAuth2AccessToken::list_all().await.unwrap();
        assert_eq!(tokens.len(), 1);

        let first = tokens.first().unwrap();
        assert_eq!(first.access_token, Some("access_token".into()));
        assert_eq!(first.refresh_token, Some("refresh_token".into()));
    }
}
