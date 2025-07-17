use crate::modules::account::entity::AuthType;
use crate::modules::cache::imap::sync::execute_account_sync;
use crate::modules::oauth2::token::OAuth2AccessToken;
use crate::modules::scheduler::periodic::TaskHandle;
use crate::modules::{
    account::{dispatcher::STATUS_DISPATCHER, entity::Account},
    error::RustMailerResult,
    scheduler::periodic::PeriodicTask,
};
use crate::utc_now;
use dashmap::DashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::{sync::LazyLock, time::Duration};
use tracing::{error, warn};

static _DESCRIPTION: &str = "This task periodically synchronizes mailbox data for a specified account, ensuring that all local data is up-to-date.";
const TASK_INTERVAL: Duration = Duration::from_secs(10);
pub static IMAP_TASKS: LazyLock<AccountSyncTask> = LazyLock::new(AccountSyncTask::new);
static LAST_WARN_TIME: AtomicI64 = AtomicI64::new(0);
const WARN_INTERVAL_MS: i64 = 600_000;

pub struct AccountSyncTask {
    tasks: DashMap<u64, TaskHandle>,
}

impl AccountSyncTask {
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
        }
    }

    pub async fn start_account_task(&self, account_id: u64, email: String) {
        let task_name = format!("account-sync-task-{}-{}", account_id, &email);
        let periodic_task = PeriodicTask::new(&task_name);
        let task = move |param: Option<u64>| {
            let account_id = param.unwrap();
            Box::pin(async move {
                let account = Account::get(account_id).await.ok();
                match account {
                    Some(account) => {
                        if !account.enabled {
                            let last = LAST_WARN_TIME.load(Ordering::Relaxed);
                            let now = utc_now!();
                            if now - last >= WARN_INTERVAL_MS {
                                LAST_WARN_TIME.store(now, Ordering::Relaxed);
                                warn!(
                                    "Account {}: Sync aborted. Account is currently disabled.",
                                    account_id
                                );
                            }
                        } else {
                            if let AuthType::OAuth2 = account.imap.auth.auth_type {
                                if OAuth2AccessToken::get(account.id).await?.is_none() {
                                    if utc_now!() % 300_000 == 0 {
                                        warn!("Account {}: Sync aborted. OAuth2 authorization not completed. Please visit the rustmailer admin page to authorize this account.", account_id);
                                    }
                                    return Ok(());
                                }
                            }
                            if let Err(e) = execute_account_sync(&account).await {
                                STATUS_DISPATCHER
                                    .append_error(
                                        account_id,
                                        format!("error in account sync task: {:#?}", e),
                                    )
                                    .await;
                                error!(
                                    "Failed to synchronize mailbox data for '{}': {:?}",
                                    account_id, e
                                )
                            }
                        }
                    }
                    None => {
                        error!(
                            "Account {}: Sync aborted. Account entity not found.",
                            account_id
                        );
                    }
                }
                Ok(())
            })
        };
        let handler = periodic_task.start(task, Some(account_id), TASK_INTERVAL, true, true);
        self.tasks.insert(account_id, handler);
    }

    pub async fn stop(&self, account_id: u64) -> RustMailerResult<()> {
        if let Some((_, handler)) = self.tasks.remove(&account_id) {
            handler.cancel().await;
        } else {
            warn!("No sync task found for account: {}", account_id);
        }
        Ok(())
    }
}
