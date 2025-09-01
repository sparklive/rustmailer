// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    modules::{
        cache::vendor::gmail::model::{
            history::HistoryList,
            labels::{LabelDetail, LabelList},
            messages::{MessageList, MessageMeta},
        },
        error::{code::ErrorCode, RustMailerResult},
        hook::http::HttpClient,
        oauth2::token::OAuth2AccessToken,
    },
    raise_error,
};
pub struct GmailClient;

impl GmailClient {
    async fn get_access_token(account_id: u64) -> RustMailerResult<String> {
        let record = OAuth2AccessToken::get(account_id).await?;
        record.and_then(|r| r.access_token).ok_or_else(|| {
            raise_error!(
                "Gmail API requires an OAuth2 access token, but authorization is incomplete."
                    .into(),
                ErrorCode::MissingConfiguration
            )
        })
    }

    pub async fn list_labels(
        account_id: u64,
        use_proxy: Option<u64>,
    ) -> RustMailerResult<LabelList> {
        let url = "https://gmail.googleapis.com/gmail/v1/users/me/labels";
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url, &access_token).await?;
        let list = serde_json::from_value::<LabelList>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Gmail API response into LabelList: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(list)
    }

    pub async fn get_label(
        account_id: u64,
        use_proxy: Option<u64>,
        label_id: &str,
    ) -> RustMailerResult<LabelDetail> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/labels/{}",
            label_id
        );
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let detail = serde_json::from_value::<LabelDetail>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Gmail API response into LabelDetail: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(detail)
    }

    pub async fn list_messages(
        account_id: u64,
        use_proxy: Option<u64>,
        label_id: &str,
        page_token: Option<String>,
        after: Option<&str>,
        max_results: u32,
    ) -> RustMailerResult<MessageList> {
        let mut url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages?labelIds={}&maxResults={}",
            label_id, max_results
        );

        if let Some(after) = after {
            url.push_str(&format!("&q=after:{}", after));
        }

        if let Some(page_token) = page_token {
            url.push_str(&format!("&pageToken={}", page_token));
        }

        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let list = serde_json::from_value::<MessageList>(value).map_err(|e| {
        raise_error!(
            format!(
                "Failed to deserialize Gmail API response into MessageList: {:#?}. Possible model mismatch or API change.",
                e
            ),
            ErrorCode::InternalError
        )
    })?;
        Ok(list)
    }

    pub async fn get_messages(
        account_id: u64,
        use_proxy: Option<u64>,
        mid: &str,
    ) -> RustMailerResult<MessageMeta> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}?format=metadata&metadataHeaders=Message-ID&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Cc&metadataHeaders=Bcc&metadataHeaders=Subject&metadataHeaders=Date&metadataHeaders=Mime-Version&metadataHeaders=Reply-To&metadataHeaders=In-Reply-To&metadataHeaders=References&metadataHeaders=Sender",
            mid
        );

        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let message = serde_json::from_value::<MessageMeta>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Gmail API response into MessageMeta: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(message)
    }

    pub async fn list_history(
        account_id: u64,
        use_proxy: Option<u64>,
        label_id: &str,
        start_history_id: &str,
        page_token: Option<&str>,
        max_results: u32,
    ) -> RustMailerResult<HistoryList> {
        let mut url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/history?labelIds={}&maxResults={}&startHistoryId={}",
            label_id, max_results, start_history_id
        );

        if let Some(page_token) = page_token {
            url.push_str(&format!("&pageToken={}", page_token));
        }

        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let list = serde_json::from_value::<HistoryList>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Gmail API response into ListMessagesResponse: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(list)
    }
}
