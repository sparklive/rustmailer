use std::sync::Arc;

use poem_grpc::Request;

use crate::{
    modules::{
        common::auth::ClientContext,
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error,
};

pub fn require_account_access<T, F>(
    request: Request<T>,
    extract_account_id: F,
) -> RustMailerResult<T>
where
    F: Fn(&T) -> u64,
{
    let extensions = request.extensions().clone();
    let context = extensions
        .get::<Arc<ClientContext>>()
        .ok_or_else(|| raise_error!("Missing ClientContext".into(), ErrorCode::InternalError))?;

    let inner = request.into_inner();
    let account_id = extract_account_id(&inner);
    context.require_account_access(account_id)?;

    Ok(inner)
}

pub fn require_root<T>(request: Request<T>) -> RustMailerResult<T> {
    let extensions = request.extensions().clone();
    let context = extensions
        .get::<Arc<ClientContext>>()
        .ok_or_else(|| raise_error!("Missing ClientContext".into(), ErrorCode::InternalError))?;

    let inner = request.into_inner();
    context.require_root()?;
    Ok(inner)
}
