use crate::modules::context::status::RustMailerStatus;
use poem::{handler, web::Json, IntoResponse};

#[handler]
pub async fn get_status() -> impl IntoResponse {
    Json(RustMailerStatus::get())
}
