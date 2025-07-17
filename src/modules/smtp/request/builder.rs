use crate::modules::error::RustMailerResult;

pub trait EmailBuilder {
    async fn validate(&self) -> RustMailerResult<()>;
    async fn build(&self, account_id: u64) -> RustMailerResult<()>;
}
