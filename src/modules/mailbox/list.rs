use crate::modules::account::entity::Account;
use crate::modules::cache::imap::mailbox::{Attribute, AttributeEnum, MailBox};
use crate::modules::context::executors::RUST_MAIL_CONTEXT;
use crate::modules::error::code::ErrorCode;
use crate::modules::error::{RustMailerError, RustMailerResult};
use crate::modules::utils::mailbox_id;
use crate::raise_error;
use async_imap::types::Name;

pub async fn get_account_mailboxes(
    account_id: u64,
    remote: bool,
) -> RustMailerResult<Vec<MailBox>> {
    let account = Account::check_account_active(account_id).await?;
    let remote = remote || account.minimal_sync;
    if remote {
        request_imap_all_mailbox_list(account_id).await
    } else {
        MailBox::list_all(account_id).await
    }
}

pub async fn list_subscribed_mailboxes(account_id: u64) -> RustMailerResult<Vec<MailBox>> {
    Account::check_account_active(account_id).await?;
    request_imap_subscribed_mailbox_list(account_id).await
}

pub async fn request_imap_subscribed_mailbox_list(
    account_id: u64,
) -> RustMailerResult<Vec<MailBox>> {
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    let names = executor.list_all_subscribed_mailboxes().await?;
    convert_names_to_mailboxes(account_id, names.iter()).await
}

pub async fn request_imap_all_mailbox_list(account_id: u64) -> RustMailerResult<Vec<MailBox>> {
    let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    let names = executor.list_all_mailboxes().await?;
    convert_names_to_mailboxes(account_id, names.iter()).await
}

fn contains_no_select(attributes: &[Attribute]) -> bool {
    attributes
        .iter()
        .any(|attr| attr.attr == AttributeEnum::NoSelect)
}

pub async fn convert_names_to_mailboxes(
    account_id: u64,
    names: impl IntoIterator<Item = &Name>,
) -> RustMailerResult<Vec<MailBox>> {
    // Preallocate enough space in the vector to avoid multiple reallocations
    let mut tasks = Vec::new();

    for name in names.into_iter() {
        // Convert the name into a MailBox structure
        let mailbox_name = name.name().to_string();
        let mut mailbox: MailBox = name.into();

        if contains_no_select(&mailbox.attributes) {
            continue;
        }
        mailbox.account_id = account_id;
        mailbox.id = mailbox_id(account_id, &mailbox.name);
        let task: tokio::task::JoinHandle<Result<MailBox, RustMailerError>> =
            tokio::spawn(async move {
                let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
                let mx = executor.examine_mailbox(mailbox_name.as_str()).await?;
                // Update the mailbox status information
                mailbox.exists = mx.exists; // Number of messages in the mailbox
                mailbox.unseen = mx.unseen; // Number of unseen messages
                mailbox.uid_next = mx.uid_next; // Next unique identifier to be assigned
                mailbox.uid_validity = mx.uid_validity; // Validity of the UIDs
                mailbox.highest_modseq = mx.highest_modseq; // Highest modification sequence number
                                                            // Collect flags and permanent flags using map
                mailbox.flags = mx.flags.into_iter().map(Into::into).collect(); // Convert flags to MailBox format
                mailbox.permanent_flags = mx.permanent_flags.into_iter().map(Into::into).collect(); // Convert permanent flags
                Ok(mailbox)
            });
        tasks.push(task);
    }

    let mut mailboxes = Vec::new();

    for task in tasks {
        match task.await {
            Ok(Ok(mailbox)) => mailboxes.push(mailbox),
            Ok(Err(err)) => return Err(err), // Handle mailbox-level errors
            Err(e) => return Err(raise_error!(format!("{:#?}", e), ErrorCode::InternalError)), // Handle task-level panics or errors
        }
    }

    Ok(mailboxes)
}
