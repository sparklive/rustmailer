// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::current_datetime;
use crate::modules::cache::imap::envelope::EmailEnvelope;
use crate::modules::common::auth::ClientContext;
use crate::modules::message::attachment::{retrieve_email_attachment, AttachmentRequest};
use crate::modules::message::content::{
    retrieve_email_content, MessageContent, MessageContentRequest,
};
use crate::modules::message::copy::{copy_mailbox_messages, MailboxTransferRequest};
use crate::modules::message::delete::{
    move_to_trash_or_delete_messages_directly, MessageDeleteRequest,
};
use crate::modules::message::flag::{modify_flags, FlagMessageRequest};
use crate::modules::message::full::retrieve_full_email;
use crate::modules::message::list::list_messages_in_mailbox;
use crate::modules::message::mv::move_mailbox_messages;
use crate::modules::message::search::payload::{MessageSearchRequest, UnifiedSearchRequest};
use crate::modules::rest::api::ApiTags;
use crate::modules::rest::response::DataPage;
use crate::modules::rest::ApiResult;
use poem::web::Path;
use poem::Body;
use poem_openapi::param::Query;
use poem_openapi::payload::{Attachment, AttachmentType, Json};
use poem_openapi::OpenApi;

pub struct MessageApi;

#[OpenApi(prefix_path = "/api/v1", tag = "ApiTags::Message")]
impl MessageApi {
    /// Moves messages from one mailbox to another for the specified account.
    #[oai(
        path = "/move-messages/:account_id",
        method = "post",
        operation_id = "move_messages"
    )]
    async fn move_messages(
        &self,
        /// The ID of the account owning the mailboxes.
        account_id: Path<u64>,
        /// specifying the source and destination mailboxes and messages
        payload: Json<MailboxTransferRequest>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(move_mailbox_messages(account_id, &payload.0).await?)
    }

    /// Copies messages from one mailbox to another for the specified account.
    #[oai(
        path = "/copy-messages/:account_id",
        method = "post",
        operation_id = "copy_messages"
    )]
    async fn copy_messages(
        &self,
        /// The ID of the account owning the mailboxes.
        account_id: Path<u64>,
        /// specifying the source and destination mailboxes and messages.
        payload: Json<MailboxTransferRequest>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(copy_mailbox_messages(account_id, &payload.0).await?)
    }

    /// Deletes messages from a mailbox or moves them to the trash for the specified account.
    #[oai(
        path = "/delete-messages/:account_id",
        method = "post",
        operation_id = "delete_messages"
    )]
    async fn delete_messages(
        &self,
        /// The ID of the account owning the mailboxes.
        account_id: Path<u64>,
        /// specifying the mailbox and messages to delete.
        payload: Json<MessageDeleteRequest>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(move_to_trash_or_delete_messages_directly(account_id, &payload.0).await?)
    }

    /// Updates flags on messages in a mailbox for the specified account.
    #[oai(
        path = "/flag-messages/:account_id",
        method = "post",
        operation_id = "update_message_flags"
    )]
    async fn update_message_flags(
        &self,
        /// The ID of the account owning the mailbox.
        account_id: Path<u64>,
        /// specifying the mailbox, messages, and flags to modify.
        payload: Json<FlagMessageRequest>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(modify_flags(account_id, payload.0).await?)
    }

    /// Lists messages in a specified mailbox for the given account.
    #[oai(
        path = "/list-messages/:account_id",
        method = "get",
        operation_id = "list_messages"
    )]
    async fn list_messages(
        &self,
        /// The ID of the account owning the mailbox.
        account_id: Path<u64>,
        /// The decoded, human-readable name of the mailbox containing the email (e.g., "INBOX").
        /// This name is presented as it appears to users, with any encoding (e.g., UTF-7) automatically handled by the system,
        /// so no manual decoding is required.
        mailbox: Query<String>,
        /// fetches messages from the IMAP server; otherwise, uses local data.
        remote: Query<Option<bool>>,
        /// The page number for pagination (1-based).
        page: Query<u64>,
        /// The number of messages per page.
        page_size: Query<u64>,
        /// lists messages in descending order; otherwise, ascending. internal date
        desc: Query<Option<bool>>,
        context: ClientContext,
    ) -> ApiResult<Json<DataPage<EmailEnvelope>>> {
        let remote = remote.0.unwrap_or(false);
        let desc = desc.0.unwrap_or(false);
        let account_id = account_id.0;
        context.require_account_access(account_id)?;

        Ok(Json(
            list_messages_in_mailbox(
                account_id,
                mailbox.0.trim(),
                page.0,
                page_size.0,
                remote,
                desc,
            )
            .await?,
        ))
    }

    /// Fetches the content of a specific email for the given account.
    #[oai(
        path = "/message-content/:account_id",
        method = "post",
        operation_id = "fetch_message_content"
    )]
    async fn fetch_message_content(
        &self,
        /// The ID of the account owning the mailbox.
        account_id: Path<u64>,
        /// specifying the mailbox and message to fetch.
        payload: Json<MessageContentRequest>,
        context: ClientContext,
    ) -> ApiResult<Json<MessageContent>> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(Json(
            retrieve_email_content(account_id, payload.0, false).await?,
        ))
    }

    /// Fetches an attachment from a specific email for the given account.
    #[oai(
        path = "/message-attachment/:account_id",
        method = "post",
        operation_id = "fetch_message_attachment"
    )]
    async fn fetch_message_attachment(
        &self,
        /// The ID of the account owning the mailbox.
        account_id: Path<u64>,
        /// specifying the mailbox, message, and attachment to fetch.
        payload: Json<AttachmentRequest>,
        context: ClientContext,
    ) -> ApiResult<Attachment<Body>> {
        let request = payload.0;
        let filename = request.attachment.filename.clone();
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        let reader = retrieve_email_attachment(account_id, request).await?;
        let body = Body::from_async_read(reader);
        let mut attachment = Attachment::new(body).attachment_type(AttachmentType::Attachment);
        if let Some(filename) = filename {
            attachment = attachment.filename(filename);
        }
        Ok(attachment)
    }

    /// Fetches the full content of a specific email for the given account.
    #[oai(
        path = "/full-message/:account_id",
        method = "get",
        operation_id = "fetch_full_message"
    )]
    async fn fetch_full_message(
        &self,
        /// The ID of the account owning the mailbox.
        account_id: Path<u64>,
        /// The decoded, human-readable name of the mailbox containing the email (e.g., "INBOX").
        /// This name is presented as it appears to users, with any encoding (e.g., UTF-7) automatically handled by the system,
        /// so no manual decoding is required.
        mailbox: Query<String>,
        /// The IMAP UID of the email to fetch.
        uid: Query<u32>,
        /// An optional filename for the attachment (defaults to a timestamped `.elm` file).
        filename: Query<Option<String>>,
        context: ClientContext,
    ) -> ApiResult<Attachment<Body>> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        let filename = filename.0.unwrap_or(format!("{}.elm", current_datetime!()));
        let reader = retrieve_full_email(account_id, mailbox.0, uid.0).await?;
        let body = Body::from_async_read(reader);
        let attachment = Attachment::new(body)
            .attachment_type(AttachmentType::Attachment)
            .filename(filename);
        Ok(attachment)
    }

    /// Searches for messages in mailboxes for the specified account. performs the search on the IMAP server;
    #[oai(
        path = "/search-message/:account_id",
        method = "post",
        operation_id = "search_messages"
    )]
    async fn search_messages(
        &self,
        /// The ID of the account owning the mailboxes.
        account_id: Path<u64>,
        /// The page number for pagination (1-based).
        page: Query<u64>,
        /// The number of messages per page.
        page_size: Query<u64>,
        /// If `true`, lists results in descending order; otherwise, ascending.
        desc: Query<Option<bool>>,
        /// specifying the search criteria (e.g., keywords, flags).
        payload: Json<MessageSearchRequest>,
        context: ClientContext,
    ) -> ApiResult<Json<DataPage<EmailEnvelope>>> {
        let request = payload.0;
        let desc = desc.0.unwrap_or(false);
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(Json(
            request
                .search(account_id, page.0, page_size.0, desc)
                .await?,
        ))
    }

    /// Searches for messages from local cache for the specified account(s).
    /// This performs a unified search across mailboxes based on indexed data,
    /// without querying the remote IMAP server.
    #[oai(
        path = "/unified-search",
        method = "post",
        operation_id = "unified_search"
    )]
    async fn unified_search(
        &self,
        /// The page number for pagination (1-based).
        page: Query<u64>,

        /// The number of messages per page.
        page_size: Query<u64>,

        /// If `true`, lists results in descending order; otherwise, ascending.
        desc: Query<Option<bool>>,

        /// The unified search criteria (email, time range, accounts, etc.).
        payload: Json<UnifiedSearchRequest>,

        /// Request context (includes authentication and permissions).
        context: ClientContext,
    ) -> ApiResult<Json<DataPage<EmailEnvelope>>> {
        let mut request = payload.0;
        let desc = desc.0.unwrap_or(false);

        // Ensure account access control
        match &mut request.accounts {
            Some(accounts) => {
                for &account_id in accounts.iter() {
                    context.require_account_access(account_id)?;
                }
            }
            None => {
                if !context.is_root {
                    // Inject accessible accounts if not specified
                    if let Some(accessible) = context.accessible_accounts()? {
                        let account_ids = accessible.iter().map(|a| a.id).collect::<Vec<u64>>();
                        request.accounts = Some(account_ids);
                    }
                }
            }
        }
        Ok(Json(request.search(page.0, page_size.0, desc).await?))
    }
}
