use crate::modules::cache::imap::mailbox::MailBox;
use crate::modules::common::auth::ClientContext;
use crate::modules::mailbox::create::create_mailbox;
use crate::modules::mailbox::delete::delete_mailbox;
use crate::modules::mailbox::list::{get_account_mailboxes, list_subscribed_mailboxes};
use crate::modules::mailbox::rename::{rename_mailbox, MailboxRenameRequest};
use crate::modules::mailbox::subscribe::{subscribe_mailbox, unsubscribe_mailbox};
use crate::modules::rest::api::ApiTags;
use crate::modules::rest::ApiResult;
use poem::web::Path;
use poem_openapi::param::Query;
use poem_openapi::payload::{Json, PlainText};
use poem_openapi::OpenApi;
pub struct MailBoxApi;

#[OpenApi(prefix_path = "/api/v1", tag = "ApiTags::Mailbox")]
impl MailBoxApi {
    /// Returns all available mailboxes for the given account.
    ///
    /// This includes both local and optionally remote (e.g., IMAP) mailboxes.
    #[oai(
        path = "/list-mailboxes/:account_id",
        method = "get",
        operation_id = "list_mailboxes"
    )]
    async fn list_mailboxes(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        /// If true, includes remote mailboxes from the server.
        remote: Query<Option<bool>>,
        context: ClientContext,
    ) -> ApiResult<Json<Vec<MailBox>>> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        let remote = remote.0.unwrap_or(false);
        Ok(Json(get_account_mailboxes(account_id, remote).await?))
    }

    /// Returns a list of mailboxes that the user is currently subscribed to.
    ///
    /// This is not a synchronized list of mail folders, but rather the subscription list
    /// as maintained by the IMAP server. In the IMAP protocol, this list reflects which
    /// mailboxes the user has chosen to subscribe to on the server side.
    #[oai(
        path = "/list-subscribed-mailboxes/:account_id",
        method = "get",
        operation_id = "list_subscribed_mailboxes"
    )]
    async fn list_subscribed_mailboxes(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        context: ClientContext,
    ) -> ApiResult<Json<Vec<MailBox>>> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(Json(list_subscribed_mailboxes(account_id).await?))
    }

    /// Subscribes to a mailbox with the specified name.
    ///
    /// This operation marks the mailbox as subscribed on the IMAP server side.
    /// It does not create or synchronize the mailbox, but only updates the
    /// server-maintained subscription list.
    #[oai(
        path = "/subscribe-mailbox/:account_id",
        method = "post",
        operation_id = "subscribe_mailbox"
    )]
    async fn subscribe_mailbox(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        /// The name of the mailbox to subscribe to.
        mailbox_name: PlainText<String>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(subscribe_mailbox(account_id, &mailbox_name).await?)
    }

    /// Unsubscribes from a mailbox with the specified name.
    ///
    /// This operation removes the mailbox from the subscription list on the IMAP server.
    /// It does not delete the mailbox or stop synchronization, but only affects the
    /// server’s record of subscribed folders.
    #[oai(
        path = "/unsubscribe-mailbox/:account_id",
        method = "post",
        operation_id = "unsubscribe_mailbox"
    )]
    async fn unsubscribe_mailbox(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        /// The name of the mailbox to unsubscribe from.
        mailbox_name: PlainText<String>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(unsubscribe_mailbox(account_id, &mailbox_name).await?)
    }

    /// Creates a new mailbox for a given account.
    #[oai(
        path = "/create-mailbox/:account_id",
        method = "post",
        operation_id = "create_mailbox"
    )]
    async fn create_mailbox(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        /// The name of the mailbox to create.
        mailbox_name: PlainText<String>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(create_mailbox(account_id, &mailbox_name).await?)
    }

    /// Deletes an existing mailbox from the specified account.
    #[oai(
        path = "/delete-mailbox/:account_id",
        method = "delete",
        operation_id = "delete_mailbox"
    )]
    async fn delete_mailbox(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        /// The name of the mailbox to delete.
        mailbox_name: PlainText<String>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(delete_mailbox(account_id, &mailbox_name).await?)
    }

    /// Renames an existing mailbox under the specified account.
    #[oai(
        path = "/rename-mailbox/:account_id",
        method = "post",
        operation_id = "rename_mailbox"
    )]
    async fn rename_mailbox(
        &self,
        /// The unique identifier of the account.
        account_id: Path<u64>,
        /// The rename payload including old and new mailbox names.
        payload: Json<MailboxRenameRequest>,
        context: ClientContext,
    ) -> ApiResult<()> {
        let account_id = account_id.0;
        context.require_account_access(account_id)?;
        Ok(rename_mailbox(account_id, payload.0).await?)
    }
}
