use std::{future::Future, sync::Arc};
use tokio::sync::Semaphore;

use crate::{
    modules::error::{code::ErrorCode, RustMailerResult},
    raise_error,
};
pub async fn run_with_limit<I, Item, Fut, F, O>(
    concurrency: usize,
    iter: I,
    f: F,
) -> RustMailerResult<Vec<O>>
where
    I: IntoIterator<Item = Item>,
    Item: Send + 'static,
    Fut: Future<Output = RustMailerResult<O>> + Send + 'static,
    F: Fn(Item) -> Fut + Send + Sync + 'static,
    O: Send + 'static,
{
    let sem = Arc::new(Semaphore::new(concurrency));
    let f = Arc::new(f);
    let mut handles = Vec::new();

    for item in iter {
        let permit = sem.clone().acquire_owned().await.map_err(|e| {
            raise_error!(
                format!("Failed to acquire semaphore: {e}"),
                ErrorCode::InternalError
            )
        })?;
        let f = f.clone();

        handles.push(tokio::spawn(async move {
            let res = f(item).await;
            drop(permit);
            res
        }));
    }

    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let res = handle.await.map_err(|e| {
            raise_error!(
                format!("Task panicked or was cancelled: {e}"),
                ErrorCode::InternalError
            )
        })?;
        results.push(res?);
    }

    Ok(results)
}
