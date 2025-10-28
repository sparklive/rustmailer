// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    base64_encode_url_safe, encode_mailbox_name,
    modules::{
        account::{entity::MailerType, migration::AccountModel},
        cache::{
            imap::{mailbox::MailBox, migration::EmailEnvelopeV3, thread::EmailThread},
            model::Envelope,
            vendor::{
                gmail::sync::{client::GmailClient, envelope::GmailEnvelope, labels::GmailLabels},
                outlook::sync::{envelope::OutlookEnvelope, folders::OutlookFolder},
            },
        },
        common::{decode_page_token, parallel::run_with_limit},
        context::executors::RUST_MAIL_CONTEXT,
        envelope::extractor::extract_envelope,
        error::{code::ErrorCode, RustMailerResult},
        rest::response::{CursorDataPage, DataPage},
    },
    raise_error,
};
use async_imap::types::Fetch;

pub async fn list_messages_in_mailbox(
    account_id: u64,
    mailbox_name: &str,
    next_page_token: Option<&str>,
    page_size: u64,
    remote: bool,
    desc: bool,
) -> RustMailerResult<CursorDataPage<Envelope>> {
    let account = AccountModel::check_account_active(account_id, false).await?;
    if page_size == 0 {
        return Err(raise_error!(
            "page_size must be greater than 0.".into(),
            ErrorCode::InvalidParameter
        ));
    }
    if page_size > 500 {
        return Err(raise_error!(
            "The page_size exceeds the maximum allowed limit of 500.".into(),
            ErrorCode::InvalidParameter
        ));
    }
    let remote = remote || account.minimal_sync();
    if remote {
        fetch_remote_messages(&account, mailbox_name, next_page_token, page_size, desc).await
    } else {
        fetch_local_messages(&account, mailbox_name, next_page_token, page_size, desc).await
    }
}

fn validate_pagination_params(page: u64, page_size: u64) -> RustMailerResult<()> {
    if page == 0 || page_size == 0 {
        return Err(raise_error!(
            "Both page and page_size must be greater than 0.".into(),
            ErrorCode::InvalidParameter
        ));
    }
    if page_size > 500 {
        return Err(raise_error!(
            "The page_size exceeds the maximum allowed limit of 500.".into(),
            ErrorCode::InvalidParameter
        ));
    }
    Ok(())
}

async fn fetch_remote_messages(
    account: &AccountModel,
    mailbox_name: &str,
    next_page_token: Option<&str>,
    page_size: u64,
    desc: bool,
) -> RustMailerResult<CursorDataPage<Envelope>> {
    match account.mailer_type {
        MailerType::ImapSmtp => {
            let page = decode_page_token(next_page_token)?;
            let excutor = RUST_MAIL_CONTEXT.imap(account.id).await?;
            let (mut fetches, total_items) = excutor
                .retrieve_metadata_paginated(
                    page,
                    page_size,
                    encode_mailbox_name!(mailbox_name).as_str(),
                    desc,
                    false,
                )
                .await?;
            if total_items == 0 {
                return Ok(CursorDataPage::new(
                    None,
                    Some(page_size),
                    0,
                    Some(0),
                    vec![],
                ));
            }

            if desc {
                fetches.reverse();
            }

            let total_pages = (total_items as f64 / page_size as f64).ceil() as u64;
            let envelopes = process_fetches(fetches, account.id, mailbox_name).await?;

            let next_page_token = if page == total_pages {
                None
            } else {
                Some(base64_encode_url_safe!((page + 1).to_string()))
            };

            Ok(CursorDataPage::new(
                next_page_token,
                Some(page_size),
                total_items,
                Some(total_pages),
                envelopes,
            ))
        }
        MailerType::GmailApi => {
            let label_map =
                GmailClient::reverse_label_map(account.id, account.use_proxy, true).await?;
            let label_id = label_map.get(mailbox_name).ok_or_else(|| {
                raise_error!(
                    format!("Label not found for mailbox: {}", mailbox_name),
                    ErrorCode::ResourceNotFound
                )
            })?;
            let message_list = GmailClient::list_messages(
                account.id,
                account.use_proxy,
                label_id,
                next_page_token,
                None,
                page_size as u32,
            )
            .await?;

            let total = message_list.result_size_estimate.ok_or_else(|| {
                raise_error!(
                    "Missing 'resultSizeEstimate' in Gmail API response".into(),
                    ErrorCode::InternalError
                )
            })?;

            let messages = message_list.messages;
            let messages = match messages {
                Some(ref msgs) if !msgs.is_empty() => msgs,
                _ => {
                    return Ok(CursorDataPage {
                        next_page_token: None,
                        page_size: Some(page_size),
                        total_items: 0,
                        items: vec![],
                        total_pages: Some(0),
                    })
                }
            };

            let account_id = account.id;
            let use_proxy = account.use_proxy;
            let next_page_token = message_list.next_page_token;
            let batch_messages =
                run_with_limit(5, messages.iter().cloned(), move |index| async move {
                    GmailClient::get_message(account_id, use_proxy, &index.id).await
                })
                .await?;

            let envelopes: Vec<Envelope> = batch_messages
                .into_iter()
                .map(|m| {
                    let mut envelope: GmailEnvelope = m.try_into()?;
                    envelope.account_id = account_id;
                    envelope.label_name = mailbox_name.into();
                    Ok(envelope.into_envelope(&label_map))
                })
                .collect::<RustMailerResult<Vec<Envelope>>>()?;

            let total_pages = (total as f64 / page_size as f64).ceil() as u64;

            Ok(CursorDataPage {
                next_page_token,
                page_size: Some(page_size),
                total_items: total,
                items: envelopes,
                total_pages: Some(total_pages),
            })
        }
        MailerType::GraphApi => todo!(),
    }
}

async fn process_fetches(
    fetches: Vec<Fetch>,
    account_id: u64,
    mailbox_name: &str,
) -> RustMailerResult<Vec<Envelope>> {
    let mut envelopes = Vec::with_capacity(fetches.len());
    for fetch in fetches {
        let envelope = extract_envelope(&fetch, account_id, mailbox_name)?;
        envelopes.push(envelope.into());
    }
    Ok(envelopes)
}

async fn fetch_local_messages(
    account: &AccountModel,
    mailbox_name: &str,
    next_page_token: Option<&str>,
    page_size: u64,
    desc: bool,
) -> RustMailerResult<CursorDataPage<Envelope>> {
    let page = decode_page_token(next_page_token)?;
    match account.mailer_type {
        MailerType::ImapSmtp => {
            let mailbox = MailBox::get(account.id, mailbox_name).await.map_err(|_| {
                raise_error!(
                    "This mailbox might not be included in the synchronized mailbox list of the account. \
                     To fetch emails from the mailbox, please add the parameter 'remote=true' in the URL."
                        .into(),
                    ErrorCode::MailBoxNotCached
                )
            })?;
            let DataPage {
                current_page: _,
                page_size,
                total_items,
                items,
                total_pages,
            } = EmailEnvelopeV3::list_messages_in_mailbox(mailbox.id, page, page_size, desc)
                .await?;

            if total_items == 0 {
                Ok(CursorDataPage::new(None, page_size, 0, None, vec![]))
            } else {
                let total_pages = total_pages.ok_or_else(|| {
                    raise_error!(
                        "Internal error: total_pages is None (this should never happen)".into(),
                        ErrorCode::InternalError
                    )
                })?;

                let next_page_token = if page == total_pages {
                    None
                } else {
                    Some(base64_encode_url_safe!((page + 1).to_string()))
                };

                Ok(CursorDataPage::new(
                    next_page_token,
                    page_size,
                    total_items,
                    Some(total_pages),
                    items.into_iter().map(Envelope::from).collect(),
                ))
            }
        }
        MailerType::GmailApi => {
            let target_label = GmailLabels::get_by_name(account.id, mailbox_name).await?;
            let DataPage {
                current_page: _,
                page_size,
                total_items,
                items,
                total_pages,
            } = GmailEnvelope::list_messages_in_label(target_label.id, page, page_size, desc)
                .await?;
            let map = GmailClient::label_map(account.id, account.use_proxy).await?;

            if total_items == 0 {
                Ok(CursorDataPage::new(None, page_size, 0, None, vec![]))
            } else {
                let total_pages = total_pages.ok_or_else(|| {
                    raise_error!(
                        "Internal error: total_pages is None (this should never happen)".into(),
                        ErrorCode::InternalError
                    )
                })?;

                let next_page_token = if page == total_pages {
                    None
                } else {
                    Some(base64_encode_url_safe!((page + 1).to_string()))
                };

                Ok(CursorDataPage::new(
                    next_page_token,
                    page_size,
                    total_items,
                    Some(total_pages),
                    items.into_iter().map(|e| e.into_envelope(&map)).collect(),
                ))
            }
        }
        MailerType::GraphApi => {
            let target_label = OutlookFolder::get_by_name(account.id, mailbox_name).await?;

            let DataPage {
                current_page: _,
                page_size,
                total_items,
                items,
                total_pages,
            } = OutlookEnvelope::list_messages_in_folder(target_label.id, page, page_size, desc)
                .await?;

            if total_items == 0 {
                Ok(CursorDataPage::new(None, page_size, 0, None, vec![]))
            } else {
                let total_pages = total_pages.ok_or_else(|| {
                    raise_error!(
                        "Internal error: total_pages is None (this should never happen)".into(),
                        ErrorCode::InternalError
                    )
                })?;

                let next_page_token = if page == total_pages {
                    None
                } else {
                    Some(base64_encode_url_safe!((page + 1).to_string()))
                };

                Ok(CursorDataPage::new(
                    next_page_token,
                    page_size,
                    total_items,
                    Some(total_pages),
                    items.into_iter().map(|e| e.into()).collect(),
                ))
            }
        }
    }
}

pub async fn list_threads_in_mailbox(
    account_id: u64,
    mailbox_name: &str,
    page: u64,
    page_size: u64,
    desc: bool,
) -> RustMailerResult<DataPage<Envelope>> {
    let account = AccountModel::check_account_active(account_id, false).await?;
    validate_pagination_params(page, page_size)?;
    if account.minimal_sync() {
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

    let not_found_err = || {
        raise_error!(
            format!(
                "Mailbox '{}' not found in the synchronized mailbox list for account {}. \
                 This may happen if the mailbox was not selected during synchronization settings \
                 or has been removed from the email server.",
                mailbox_name, account_id
            ),
            ErrorCode::MailBoxNotCached
        )
    };

    match account.mailer_type {
        MailerType::ImapSmtp => {
            let mailbox = MailBox::get(account.id, mailbox_name)
                .await
                .map_err(|_| not_found_err())?;
            EmailThread::list_threads_in_mailbox(mailbox.id, page, page_size, desc).await
        }
        MailerType::GmailApi => {
            let label = GmailLabels::get_by_name(account_id, mailbox_name).await?;
            EmailThread::list_threads_in_label(account, label.id, page, page_size, desc).await
        }
        MailerType::GraphApi => todo!(),
    }
}

pub async fn get_thread_messages(
    account_id: u64,
    thread_id: u64,
) -> RustMailerResult<Vec<Envelope>> {
    let account = AccountModel::check_account_active(account_id, false).await?;
    if account.minimal_sync() {
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

    match account.mailer_type {
        MailerType::ImapSmtp => EmailEnvelopeV3::get_thread(account_id, thread_id).await,
        MailerType::GmailApi => {
            let envelopes = GmailEnvelope::get_thread(account_id, thread_id).await?;
            let map = GmailClient::label_map(account_id, account.use_proxy).await?;
            Ok(envelopes
                .into_iter()
                .map(|e| e.into_envelope(&map))
                .collect())
        }
        MailerType::GraphApi => todo!(),
    }
}
