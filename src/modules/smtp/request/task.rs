use std::sync::Arc;
use std::time::Instant;

use crate::modules::cache::disk::DISK_CACHE;
use crate::modules::error::code::ErrorCode;
use crate::modules::error::RustMailerResult;
use crate::modules::hook::channel::{Event, EVENT_CHANNEL};
use crate::modules::hook::events::{
    payload::EmailSentSuccess, EventPayload, EventType, RustMailerEvent,
};
use crate::modules::hook::task::EventHookTask;
use crate::modules::metrics::{
    FAILURE, RUSTMAILER_EMAIL_SEND_DURATION_SECONDS, RUSTMAILER_EMAIL_SENT_BYTES,
    RUSTMAILER_EMAIL_SENT_TOTAL, SUCCESS,
};
use crate::modules::smtp::executor::SmtpExecutor;
use crate::raise_error;

use crate::modules::scheduler::{
    retry::{RetryPolicy, RetryStrategy},
    task::{Task, TaskFuture},
};

use crate::modules::smtp::{
    mta::entity::Mta,
    request::{EmailHandler, MailEnvelope, SendControl, Strategy},
};

use crate::modules::{account::entity::Account, context::executors::RUST_MAIL_CONTEXT};

use mail_send::smtp::message::{Address, Message, Parameters};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

pub const EXT_DSN: u32 = 1 << 10;
pub const OUTBOX_QUEUE: &str = "send_email";

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SmtpTask {
    pub account_id: u64,
    pub account_email: String,
    pub subject: Option<String>,
    pub message_id: String,
    pub from: String,
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub attachment_count: usize,
    pub control: SendControl,
    pub cache_key: String,
    pub answer_email: Option<AnswerEmail>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct AnswerEmail {
    pub reply: bool,
    pub mailbox: String,
    pub uid: u32,
}

impl SmtpTask {
    async fn build_message<'a>(
        envelope: &'a Option<MailEnvelope>,
        body: &'a [u8],
        from: String,
        recipients: &[String],
        dsn_params: Option<(&'a Parameters<'a>, &'a Parameters<'a>)>,
    ) -> Message<'a> {
        let mut message = Message::empty();
        message = message.body(body);
        if let Some(envelope) = envelope {
            message = message.from(if let Some((mail_params, _)) = dsn_params {
                Address::new(envelope.from.clone(), mail_params.clone())
            } else {
                Address::from(envelope.from.clone())
            });

            for recip in &envelope.recipients {
                message = message.to(if let Some((_, rcpt_params)) = dsn_params {
                    Address::new(recip.clone(), rcpt_params.clone())
                } else {
                    Address::from(recip.clone())
                });
            }
        } else {
            message = message.from(from);
            for recip in recipients {
                message = message.to(recip.clone());
            }
        }

        message
    }
}

impl Task for SmtpTask {
    const TASK_KEY: &'static str = "send_email";
    const TASK_QUEUE: &'static str = OUTBOX_QUEUE;

    //default delay seconds
    fn delay_seconds(&self) -> u32 {
        1
    }

    fn retry_policy(&self) -> RetryPolicy {
        match &self.control.retry_policy {
            Some(retry_policy) => RetryPolicy {
                strategy: match retry_policy.strategy {
                    Strategy::Linear => RetryStrategy::Linear {
                        interval: retry_policy.seconds,
                    },
                    Strategy::Exponential => RetryStrategy::Exponential {
                        base: retry_policy.seconds,
                    },
                },
                max_retries: Some(retry_policy.max_retries),
            },
            None => RetryPolicy {
                strategy: RetryStrategy::Linear { interval: 2 },
                max_retries: Some(10),
            },
        }
    }

    fn run(self, _task_id: u64) -> TaskFuture {
        Box::pin(async move {
            let start = Instant::now();
            let (executor, params) = match self.control.mta {
                Some(mta) => {
                    let mta = Mta::get(mta).await?.ok_or_else(|| {
                        raise_error!("MTA not found.".into(), ErrorCode::ResourceNotFound)
                    })?;
                    let executor = RUST_MAIL_CONTEXT.mta(mta.id).await?;
                    let params = if mta.dsn_capable {
                        let params = self.control.build_dsn_params()?;
                        Some(params)
                    } else {
                        None
                    };
                    (executor, params)
                }
                None => {
                    let account = Account::get(self.account_id).await?;
                    let executor = RUST_MAIL_CONTEXT.smtp(account.id).await?;

                    let dsn_capable = if let Some(dsn_capable) = &account.dsn_capable {
                        *dsn_capable
                    } else {
                        let capabilities = executor.capabilities(&account.smtp.host).await?;
                        let dsn_capable = capabilities & EXT_DSN != 0;
                        Account::update_dsn_capable(account.id, dsn_capable).await?;
                        dsn_capable
                    };

                    let params = if dsn_capable {
                        let params = self.control.build_dsn_params()?;
                        Some(params)
                    } else {
                        None
                    };
                    (executor, params)
                }
            };

            let mut reader = DISK_CACHE
                .get_cache(&self.cache_key)
                .await?
                .ok_or_else(|| {
                    raise_error!(
                        "failed to get cache reader to load email body.".into(),
                        ErrorCode::InternalError
                    )
                })?;
            let mut body = Vec::new();
            reader.read_to_end(&mut body).await.map_err(|e| {
                raise_error!(
                    format!("failed to load email body from disk cache. {:#?}", e),
                    ErrorCode::InternalError
                )
            })?;

            let message = if let Some((mail_params, rcpt_params)) = &params {
                Self::build_message(
                    &self.control.envelope,
                    &body,
                    self.from.clone(),
                    &self.to,
                    Some((mail_params, rcpt_params)),
                )
                .await
            } else {
                Self::build_message(
                    &self.control.envelope,
                    &body,
                    self.from.clone(),
                    &self.to,
                    None,
                )
                .await
            };
            match send_email(executor, message).await {
                Ok(()) => {
                    let elapsed = start.elapsed();
                    RUSTMAILER_EMAIL_SEND_DURATION_SECONDS
                        .with_label_values(&[SUCCESS])
                        .observe(elapsed.as_secs_f64());
                    RUSTMAILER_EMAIL_SENT_TOTAL
                        .with_label_values(&[SUCCESS])
                        .inc();
                    RUSTMAILER_EMAIL_SENT_BYTES.inc_by(body.len() as u64);
                    if EventHookTask::event_watched(self.account_id, EventType::EmailSentSuccess)
                        .await?
                    {
                        EVENT_CHANNEL
                            .queue(Event::new(
                                self.account_id,
                                &self.account_email,
                                RustMailerEvent::new(
                                    EventType::EmailSentSuccess,
                                    EventPayload::EmailSentSuccess(EmailSentSuccess {
                                        account_id: self.account_id,
                                        account_email: self.account_email.clone(),
                                        from: self.from.clone(),
                                        to: self.to.clone(),
                                        subject: self.subject.clone(),
                                        message_id: self.message_id.clone(),
                                    }),
                                ),
                            ))
                            .await;
                    }

                    if let Some(answer_email) = &self.answer_email {
                        EmailHandler::mark_message_answered(
                            self.account_id,
                            &answer_email.mailbox,
                            answer_email.uid,
                        )
                        .await?;
                    }

                    self.control
                        .save_to_sent_if_needed(self.account_id, &body)
                        .await?;

                    Ok(())
                }
                Err(e) => {
                    let elapsed = start.elapsed();
                    RUSTMAILER_EMAIL_SEND_DURATION_SECONDS
                        .with_label_values(&[FAILURE])
                        .observe(elapsed.as_secs_f64());
                    RUSTMAILER_EMAIL_SENT_TOTAL
                        .with_label_values(&[FAILURE])
                        .inc();
                    Err(e)
                }
            }
        })
    }
}

async fn send_email(executor: Arc<SmtpExecutor>, message: Message<'_>) -> RustMailerResult<()> {
    executor.send_email(message).await
}
