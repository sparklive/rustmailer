// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use std::sync::Arc;

use ahash::AHashMap;
use serde_json::json;

use crate::{
    modules::{
        cache::vendor::gmail::{
            cache::GMAIL_LABELS_CACHE,
            model::{
                history::HistoryList,
                labels::{Label, LabelDetail, LabelList},
                messages::{FullMessage, MessageList, MessageMeta, PartBody},
            },
        },
        error::{code::ErrorCode, RustMailerResult},
        hook::http::HttpClient,
        mailbox::{create::CreateMailboxRequest, rename::MailboxUpdateRequest},
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

    pub async fn list_visible_labels(
        account_id: u64,
        use_proxy: Option<u64>,
    ) -> RustMailerResult<Vec<Label>> {
        let all_labels = Self::list_labels(account_id, use_proxy).await?;
        let visible_labels: Vec<Label> = all_labels.labels;
        Ok(visible_labels)
    }

    pub async fn label_map(
        account_id: u64,
        use_proxy: Option<u64>,
    ) -> RustMailerResult<Arc<AHashMap<String, String>>> {
        if let Some(v) = GMAIL_LABELS_CACHE.get(&account_id).await {
            return Ok(v.clone());
        }
        let visible_labels = Self::list_visible_labels(account_id, use_proxy).await?;
        let map: Arc<AHashMap<String, String>> = Arc::new(
            visible_labels
                .into_iter()
                .map(|label| (label.id, label.name))
                .collect(),
        );
        GMAIL_LABELS_CACHE.set(account_id, map.clone()).await;
        Ok(map)
    }

    pub async fn reverse_label_map(
        account_id: u64,
        use_proxy: Option<u64>,
        skip_cache: bool,
    ) -> RustMailerResult<AHashMap<String, String>> {
        if !skip_cache {
            if let Some(v) = GMAIL_LABELS_CACHE.get(&account_id).await {
                let map: AHashMap<String, String> =
                    v.iter().map(|(k, v)| (v.clone(), k.clone())).collect();
                return Ok(map);
            }
        }
        let visible_labels = Self::list_visible_labels(account_id, use_proxy).await?;
        let map: Arc<AHashMap<String, String>> = Arc::new(
            visible_labels
                .into_iter()
                .map(|label| (label.id, label.name))
                .collect(),
        );
        GMAIL_LABELS_CACHE.set(account_id, map.clone()).await;
        let map: AHashMap<String, String> =
            map.iter().map(|(k, v)| (v.clone(), k.clone())).collect();
        Ok(map)
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

    pub async fn create_label(
        account_id: u64,
        use_proxy: Option<u64>,
        request: &CreateMailboxRequest,
    ) -> RustMailerResult<()> {
        let url = "https://gmail.googleapis.com/gmail/v1/users/me/labels";
        let client = HttpClient::new(use_proxy).await?;

        let mut body = json!({
            "name": request.mailbox_name,
            "messageListVisibility": "show",
            "labelListVisibility": "labelShow",
            "type": "user"
        });
        if let Some(color) = &request.label_color {
            body["color"] = json!({
                "textColor": color.text_color,
                "backgroundColor": color.background_color
            });
        }
        let access_token = Self::get_access_token(account_id).await?;
        client.post(url, &access_token, Some(&body)).await?;
        Ok(())
    }

    pub async fn delete_label(
        account_id: u64,
        use_proxy: Option<u64>,
        label_id: &str,
    ) -> RustMailerResult<()> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/labels/{}",
            label_id
        );
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        client.delete(url.as_str(), &access_token).await?;
        Ok(())
    }

    pub async fn update_label(
        account_id: u64,
        use_proxy: Option<u64>,
        label_id: &str,
        request: &MailboxUpdateRequest,
    ) -> RustMailerResult<()> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/labels/{}",
            label_id
        );

        let mut body = json!({
            "id": label_id,
            "name": request.new_name,
            "messageListVisibility": "show",
            "labelListVisibility": "labelShow",
            "type": "user"
        });
        if let Some(color) = &request.label_color {
            body["color"] = json!({
                "textColor": color.text_color,
                "backgroundColor": color.background_color
            });
        }

        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        client.put(url.as_str(), &access_token, &body).await?;
        Ok(())
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

    pub async fn move_to_trash(
        account_id: u64,
        use_proxy: Option<u64>,
        mid: &str,
    ) -> RustMailerResult<()> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}/trash",
            mid
        );
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        client
            .post(url.as_str(), &access_token, None::<&()>)
            .await?;
        Ok(())
    }

    pub async fn get_full_messages(
        account_id: u64,
        use_proxy: Option<u64>,
        mid: &str,
    ) -> RustMailerResult<FullMessage> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}?format=full",
            mid
        );
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let message = serde_json::from_value::<FullMessage>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Gmail API response into FullMessage: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(message)
    }

    pub async fn get_attachments(
        account_id: u64,
        use_proxy: Option<u64>,
        mid: &str,
        aid: &str,
    ) -> RustMailerResult<PartBody> {
        let url = format!(
            "https://gmail.googleapis.com/gmail/v1/users/me/messages/{}/attachments/{}",
            mid, aid
        );
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let result = serde_json::from_value::<PartBody>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Gmail API response into PartBody: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(result)
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
            "https://gmail.googleapis.com/gmail/v1/users/me/history?labelId={}&maxResults={}&startHistoryId={}",
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

    pub async fn create_draft(
        account_id: u64,
        use_proxy: Option<u64>,
        body: serde_json::Value,
    ) -> RustMailerResult<serde_json::Value> {
        let url = "https://gmail.googleapis.com/gmail/v1/users/me/drafts";
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.post(url, &access_token, Some(&body)).await?;
        Ok(value)
    }
}
