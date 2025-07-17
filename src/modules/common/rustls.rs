use crate::{
    modules::{
        context::Initialize,
        error::{code::ErrorCode, RustMailerResult},
    },
    raise_error,
};

pub struct RustMailerTls;

impl Initialize for RustMailerTls {
    async fn initialize() -> RustMailerResult<()> {
        rustls::crypto::CryptoProvider::install_default(rustls::crypto::ring::default_provider())
            .map_err(|_| {
                raise_error!(
                    "failed to set crypto provider".into(),
                    ErrorCode::InternalError
                )
            })
    }
}
 