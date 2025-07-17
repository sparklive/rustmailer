use crate::modules::error::RustMailerResult;

pub mod controller;
pub mod executors;
pub mod status;

pub trait Initialize {
    async fn initialize() -> RustMailerResult<()>;
}

pub trait RustMailTask {
    fn start();
}
