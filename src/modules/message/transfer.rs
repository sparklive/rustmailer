// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::{
    encode_mailbox_name,
    modules::{
        account::{entity::MailerType, v2::AccountV2},
        cache::vendor::gmail::sync::client::GmailClient,
        context::executors::RUST_MAIL_CONTEXT,
        envelope::generate_uid_set,
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error,
};
use poem_openapi::Object;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize, Object)]
pub struct MailboxTransferRequest {
    /// A list of unique message identifiers as strings.
    ///
    /// - For IMAP accounts, each UID is converted to a numeric string (parseable back to `u32`).
    /// - For Gmail API accounts, each element is a message ID (`mid`) returned by the API.
    /// Unifying them as strings simplifies handling across different backends.
    pub ids: Vec<String>,
    /// The name of the mailbox from which the messages will be moved.
    /// For IMAP: the decoded, human-readable name of the mailbox (e.g., "INBOX").
    /// For Gmail API: represents the label name.
    pub current_mailbox: String,
    /// The name of the mailbox to which the messages will be moved.
    /// For IMAP: the decoded, human-readable name of the mailbox (e.g., "INBOX").
    /// For Gmail API: represents the label name.
    pub target_mailbox: String,
}

#[derive(Clone, Default, Debug)]
pub enum MessageTransfer {
    #[default]
    Move,
    Copy,
}

pub async fn transfer_messages(
    account_id: u64,
    payload: &MailboxTransferRequest,
    transfer: MessageTransfer,
) -> RustMailerResult<()> {
    // Ensure the account exists before proceeding
    let account = AccountV2::check_account_active(account_id, false).await?;

    match account.mailer_type {
        MailerType::ImapSmtp => {
            if payload.ids.is_empty() {
                return Err(raise_error!(
                    "`ids` must contain at least one element".into(),
                    ErrorCode::InvalidParameter
                ));
            }

            let uids: Vec<u32> = payload
                .ids
                .iter()
                .map(|id| {
                    id.parse::<u32>().map_err(|_| {
                        raise_error!(
                            format!("Invalid IMAP UID: '{}', must be a numeric string", id),
                            ErrorCode::InvalidParameter
                        )
                    })
                })
                .collect::<Result<_, _>>()?;

            let uid_set = generate_uid_set(uids);
            let executor = RUST_MAIL_CONTEXT.imap(account_id).await?;
            // Encode the mailbox names using UTF-7 encoding
            let current_mailbox = encode_mailbox_name!(payload.current_mailbox.clone());
            let target_mailbox = encode_mailbox_name!(payload.target_mailbox.clone());

            match transfer {
                MessageTransfer::Move => {
                    // Move the messages from the current mailbox to the target mailbox
                    executor
                        .uid_move_envelopes(
                            uid_set.as_str(),
                            current_mailbox.as_str(),
                            target_mailbox.as_str(),
                        )
                        .await
                }
                MessageTransfer::Copy => {
                    // Copy the messages from the current mailbox to the target mailbox
                    executor
                        .uid_copy_envelopes(
                            uid_set.as_str(),
                            current_mailbox.as_str(),
                            target_mailbox.as_str(),
                        )
                        .await
                }
            }
        }
        MailerType::GmailApi => {
            let mids = &payload.ids;

            if mids.is_empty() {
                return Err(raise_error!(
                    "Gmail API copy requires at least one message ID".into(),
                    ErrorCode::InvalidParameter
                ));
            }

            if mids.len() > 500 {
                return Err(raise_error!(
                    format!(
                        "Gmail API batchModify supports at most 500 message IDs, got {}",
                        mids.len()
                    ),
                    ErrorCode::InvalidParameter
                ));
            }

            let labels_map =
                GmailClient::reverse_label_map(account_id, account.use_proxy, true).await?;

            match transfer {
                MessageTransfer::Move => {
                    let target_label_id =
                        labels_map.get(&payload.target_mailbox).ok_or_else(|| {
                            raise_error!(
                                format!(
                                    "Target mailbox/label `{}` not found in Gmail labels",
                                    payload.target_mailbox
                                ),
                                ErrorCode::InvalidParameter
                            )
                        })?;

                    let current_label_id =
                        labels_map.get(&payload.current_mailbox).ok_or_else(|| {
                            raise_error!(
                                format!(
                                    "Current mailbox/label `{}` not found in Gmail labels",
                                    payload.current_mailbox
                                ),
                                ErrorCode::InvalidParameter
                            )
                        })?;
                    //modify label
                    GmailClient::batch_modify(
                        account_id,
                        account.use_proxy,
                        mids,
                        vec![target_label_id.into()],
                        vec![current_label_id.into()],
                    )
                    .await
                }
                MessageTransfer::Copy => {
                    let target_label_id =
                        labels_map.get(&payload.target_mailbox).ok_or_else(|| {
                            raise_error!(
                                format!(
                                    "Target mailbox/label `{}` not found in Gmail labels",
                                    payload.target_mailbox
                                ),
                                ErrorCode::InvalidParameter
                            )
                        })?;
                    //add label
                    GmailClient::batch_modify(
                        account_id,
                        account.use_proxy,
                        mids,
                        vec![target_label_id.into()],
                        vec![],
                    )
                    .await
                }
            }
        }
    }
}
