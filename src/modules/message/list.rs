// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    encode_mailbox_name,
    modules::{
        account::entity::Account,
        cache::imap::{envelope_v2::EmailEnvelopeV2, mailbox::MailBox, thread::EmailThread},
        context::executors::RUST_MAIL_CONTEXT,
        envelope::extractor::extract_envelope,
        error::{code::ErrorCode, RustMailerResult},
        rest::response::DataPage,
    },
    raise_error,
};
use async_imap::types::Fetch;

pub async fn list_messages_in_mailbox(
    account_id: u64,
    mailbox_name: &str,
    page: u64,
    page_size: u64,
    remote: bool,
    desc: bool,
) -> RustMailerResult<DataPage<EmailEnvelopeV2>> {
    let account = Account::check_account_active(account_id).await?;
    validate_pagination_params(page, page_size)?;
    let remote = remote || account.minimal_sync;

    if remote {
        fetch_remote_messages(account_id, mailbox_name, page, page_size, desc).await
    } else {
        fetch_local_messages(&account, mailbox_name, page, page_size, desc).await
    }
}

fn validate_pagination_params(page: u64, page_size: u64) -> RustMailerResult<()> {
    if page == 0 || page_size == 0 {
        return Err(raise_error!(
            "Both page and page_size must be greater than 0.".into(),
            ErrorCode::InvalidParameter
        ));
    }
    if page_size > 1000 {
        return Err(raise_error!(
            "The page_size exceeds the maximum allowed limit of 1000.".into(),
            ErrorCode::InvalidParameter
        ));
    }
    Ok(())
}

async fn fetch_remote_messages(
    account_id: u64,
    mailbox_name: &str,
    page: u64,
    page_size: u64,
    desc: bool,
) -> RustMailerResult<DataPage<EmailEnvelopeV2>> {
    let excutor = RUST_MAIL_CONTEXT.imap(account_id).await?;
    let (mut fetches, total_items) = excutor
        .retrieve_metadata_paginated(
            page,
            page_size,
            encode_mailbox_name!(mailbox_name).as_str(),
            desc,
            false,
        )
        .await?;

    if desc {
        fetches.reverse();
    }

    let total_pages = (total_items as f64 / page_size as f64).ceil() as u64;
    let envelopes = process_fetches(fetches, account_id, mailbox_name).await?;

    Ok(DataPage::new(
        Some(page),
        Some(page_size),
        total_items,
        Some(total_pages),
        envelopes,
    ))
}

async fn process_fetches(
    fetches: Vec<Fetch>,
    account_id: u64,
    mailbox_name: &str,
) -> RustMailerResult<Vec<EmailEnvelopeV2>> {
    let mut envelopes = Vec::with_capacity(fetches.len());
    for fetch in fetches {
        let envelope = extract_envelope(&fetch, account_id, mailbox_name)?;
        envelopes.push(envelope);
    }
    Ok(envelopes)
}

async fn fetch_local_messages(
    account: &Account,
    mailbox_name: &str,
    page: u64,
    page_size: u64,
    desc: bool,
) -> RustMailerResult<DataPage<EmailEnvelopeV2>> {
    match MailBox::get(account.id, mailbox_name).await {
        Ok(mailbox) => {
            EmailEnvelopeV2::list_messages_in_mailbox(mailbox.id, page, page_size, desc).await
        }
        Err(_) => Err(raise_error!(
            "This mailbox might not be included in the synchronized mailbox list of the account. \
            To fetch emails from the mailbox, please add the parameter 'remote=true' in the URL."
                .into(),
            ErrorCode::MailBoxNotCached
        )),
    }
}

pub async fn list_threads_in_mailbox(
    account_id: u64,
    mailbox_name: &str,
    page: u64,
    page_size: u64,
    desc: bool,
) -> RustMailerResult<DataPage<EmailEnvelopeV2>> {
    let account = Account::check_account_active(account_id).await?;
    validate_pagination_params(page, page_size)?;
    if account.minimal_sync {
        return Err(raise_error!(
            format!(
                "Account {} is in minimal sync mode. Listing threads in a mailbox is not supported. \
                To enable this feature, you must delete the email account configuration and set it up again \
                with minimal sync mode disabled.",
                account_id
            ),
            ErrorCode::Incompatible
        ));
    }

    match MailBox::get(account.id, mailbox_name).await {
        Ok(mailbox) => {
            EmailThread::list_threads_in_mailbox(mailbox.id, page, page_size, desc).await
        }
        Err(_) => Err(raise_error!(
            format!(
                "Mailbox '{}' not found in the synchronized mailbox list for account {}. \
                This may happen if the mailbox was not selected during synchronization settings \
                or has been removed from the email server.",
                mailbox_name, account_id
            ),
            ErrorCode::MailBoxNotCached
        )),
    }
}

pub async fn get_thread_messages(
    account_id: u64,
    mailbox_name: &str,
    thread_id: u64,
) -> RustMailerResult<Vec<EmailEnvelopeV2>> {
    let account = Account::check_account_active(account_id).await?;
    if account.minimal_sync {
        return Err(raise_error!(
            format!(
                "Account {} is in minimal sync mode. Listing threads in a mailbox is not supported. \
                To enable this feature, you must delete the email account configuration and set it up again \
                with minimal sync mode disabled.",
                account_id
            ),
            ErrorCode::Incompatible
        ));
    }

    match MailBox::get(account.id, mailbox_name).await {
        Ok(mailbox) => EmailEnvelopeV2::get_thread(account_id, mailbox.id, thread_id).await,
        Err(_) => Err(raise_error!(
            format!(
                "Mailbox '{}' not found in the synchronized mailbox list for account {}. \
                This may happen if the mailbox was not selected during synchronization settings \
                or has been removed from the email server.",
                mailbox_name, account_id
            ),
            ErrorCode::MailBoxNotCached
        )),
    }
}
