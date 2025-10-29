use crate::{
    modules::{
        cache::vendor::outlook::model::{
            MailFolder, MailFoldersResponse, Message, MessageListResponse,
        },
        error::{code::ErrorCode, RustMailerResult},
        hook::http::HttpClient,
        oauth2::token::OAuth2AccessToken,
    },
    raise_error,
};
use std::{future::Future, pin::Pin};

pub struct OutlookClient;

impl OutlookClient {
    pub async fn get_access_token(account_id: u64) -> RustMailerResult<String> {
        let record = OAuth2AccessToken::get(account_id).await?;
        record.and_then(|r| r.access_token).ok_or_else(|| {
            raise_error!(
                "Graph API requires an OAuth2 access token, but authorization is incomplete."
                    .into(),
                ErrorCode::MissingConfiguration
            )
        })
    }

    async fn fetch_mailfolders_page(
        client: &HttpClient,
        url: &str,
        access_token: &str,
    ) -> RustMailerResult<MailFoldersResponse> {
        let value = client.get(url, access_token).await.map_err(|e| {
            raise_error!(format!("Request error: {e:#?}"), ErrorCode::InternalError)
        })?;
        let folders = serde_json::from_value::<MailFoldersResponse>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Graph API response into MailFoldersResponse: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(folders)
    }

    fn fetch_recursive<'a>(
        client: &'a HttpClient,
        folder_id: Option<&'a str>,
        prefix: &'a str,
        output: &'a mut Vec<MailFolder>,
        access_token: &'a str,
    ) -> Pin<Box<dyn Future<Output = RustMailerResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let mut url = match folder_id {
                Some(id) => {
                    format!("https://graph.microsoft.com/v1.0/me/mailFolders/{id}/childFolders")
                }
                None => "https://graph.microsoft.com/v1.0/me/mailFolders".to_string(),
            };
            loop {
                let resp = Self::fetch_mailfolders_page(client, &url, access_token).await?;
                for mut folder in resp.value {
                    let full_name = if prefix.is_empty() {
                        folder.display_name.clone()
                    } else {
                        format!("{}/{}", prefix, folder.display_name)
                    };
                    folder.display_name = full_name.clone();
                    output.push(folder.clone());
                    if folder.child_folder_count.unwrap_or(0) > 0 {
                        Self::fetch_recursive(
                            client,
                            Some(&folder.id),
                            &full_name,
                            output,
                            access_token,
                        )
                        .await?;
                    }
                }
                if let Some(next) = resp.next_link {
                    url = next;
                } else {
                    break;
                }
            }
            Ok(())
        })
    }

    pub async fn get_folder(
        account_id: u64,
        use_proxy: Option<u64>,
        default_folder_name: &str,
    ) -> RustMailerResult<MailFolder> {
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let url = format!("https://graph.microsoft.com/v1.0/me/mailFolders/{default_folder_name}");
        let value = client.get(&url, &access_token).await.map_err(|e| {
            raise_error!(format!("Request error: {e:#?}"), ErrorCode::InternalError)
        })?;
        let folder = serde_json::from_value::<MailFolder>(value)
            .map_err(|e| raise_error!(format!(
                "Failed to deserialize Graph API response into MailFolder: {:#?}. Possible model mismatch or API change.",
                e
            ), ErrorCode::InternalError))?;
        Ok(folder)
    }

    pub async fn list_mailfolders(
        account_id: u64,
        use_proxy: Option<u64>,
    ) -> RustMailerResult<Vec<MailFolder>> {
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let mut result = Vec::new();
        Self::fetch_recursive(&client, None, "", &mut result, &access_token).await?;
        Ok(result)
    }

    pub async fn list_messages(
        account_id: u64,
        use_proxy: Option<u64>,
        folder_id: &str,
        page: u32,
        page_size: u32,
        after: Option<&str>,
    ) -> RustMailerResult<MessageListResponse> {
        assert!(page > 0, "page must be greater than 0");
        let skip = (page - 1) * page_size;
        let base_url = format!(
            "https://graph.microsoft.com/v1.0/me/mailFolders/{folder_id}/messages?\
            $top={page_size}&\
            $skip={skip}&\
            $orderBy=receivedDateTime desc&\
            $select=id,isRead,conversationId,internetMessageId,from,body,toRecipients,ccRecipients,\
            bccRecipients,replyTo,sender,subject,receivedDateTime,sentDateTime,isRead,bodyPreview,categories&\
            $expand=attachments($select=id,name,contentType,size,isInline,microsoft.graph.fileAttachment/contentId)",
            folder_id = folder_id,
            page_size = page_size,
            skip = skip,
        );

        let url = if let Some(after) = after {
            format!("{}&$filter=receivedDateTime ge {}", base_url, after)
        } else {
            base_url
        };

        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let list = serde_json::from_value::<MessageListResponse>(value).map_err(|e| {
            raise_error!(
                format!(
                    "Failed to deserialize Graph API response into MessageListResponse: {:#?}. Possible model mismatch or API change.",
                    e
                ),
                ErrorCode::InternalError
            )
        })?;
        Ok(list)
    }

    pub async fn get_delta_link(
        account_id: u64,
        use_proxy: Option<u64>,
        folder_id: &str,
    ) -> RustMailerResult<String> {
        let mut url = format!(
            "https://graph.microsoft.com/v1.0/me/mailFolders/{folder_id}/messages/delta?$select=id"
        );
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        loop {
            let value = client.get(url.as_str(), &access_token).await?;
            if let Some(next_link) = value.get("@odata.nextLink") {
                url = next_link
                    .as_str()
                    .ok_or_else(|| {
                        raise_error!(
                            format!("unexpected type for @odata.nextLink in response at URL={url}"),
                            ErrorCode::InternalError
                        )
                    })?
                    .to_string();
            } else if let Some(delta_link) = value.get("@odata.deltaLink") {
                return Ok(delta_link
                    .as_str()
                    .ok_or_else(|| {
                        raise_error!(
                            format!(
                                "unexpected type for @odata.deltaLink in response at URL={url}"
                            ),
                            ErrorCode::InternalError
                        )
                    })?
                    .to_string());
            } else {
                return Err(raise_error!(format!(
                    "neither @odata.nextLink nor @odata.deltaLink found in Graph API response at URL={url}"
                ), ErrorCode::InternalError));
            }
        }
    }

    pub async fn get_message(
        account_id: u64,
        use_proxy: Option<u64>,
        id: &str,
    ) -> RustMailerResult<Message> {
        let url = format!("https://graph.microsoft.com/v1.0/me/messages/{id}?\
               $select=id,isRead,conversationId,internetMessageId,from,body,toRecipients,ccRecipients,\
               bccRecipients,replyTo,sender,subject,receivedDateTime,sentDateTime,isRead,bodyPreview,categories&\
               $expand=attachments($select=id,name,contentType,size,isInline,microsoft.graph.fileAttachment/contentId)");

        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let message = serde_json::from_value::<Message>(value).map_err(|e| {
            raise_error!(
                format!(
                    "Failed to deserialize Graph API response into MessageListResponse: {:#?}. Possible model mismatch or API change.",
                    e
                ),
                ErrorCode::InternalError
            )
        })?;
        Ok(message)
    }

    pub async fn get_attachment(
        account_id: u64,
        use_proxy: Option<u64>,
        mid: &str,
        aid: &str,
    ) -> RustMailerResult<String> {
        let url = format!("https://graph.microsoft.com/v1.0/me/messages/{mid}/attachments/{aid}");
        let client = HttpClient::new(use_proxy).await?;
        let access_token = Self::get_access_token(account_id).await?;
        let value = client.get(url.as_str(), &access_token).await?;
        let data = value
            .get("contentBytes")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                raise_error!(
                    "Attachment missing contentBytes — possibly a reference or item attachment."
                        .into(),
                    ErrorCode::InternalError
                )
            })?;
        Ok(data.into())
    }
}
