use crate::modules::error::code::ErrorCode;
use crate::modules::hook::entity::HttpMethod;
use crate::raise_error;
use crate::{modules::error::RustMailerResult, rustmailer_version};
use std::collections::HashMap;
use std::time::Duration;

#[cfg(test)]
mod tests;

pub struct HttpClient {
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new() -> RustMailerResult<HttpClient> {
        Ok(Self {
            client: reqwest::ClientBuilder::new()
                .user_agent(rustmailer_version!())
                .timeout(Duration::from_secs(10))
                .connect_timeout(Duration::from_secs(3))
                .build()
                .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?,
        })
    }

    pub async fn send_json_request(
        &self,
        task_info: Option<HashMap<String, String>>,
        method: HttpMethod,
        url: &str,
        payload: &serde_json::Value,
        headers: Option<HashMap<String, String>>,
    ) -> RustMailerResult<reqwest::Response> {
        let mut request_builder = match method {
            HttpMethod::Post => self.client.post(url),
            HttpMethod::Put => self.client.put(url),
        };
        request_builder = request_builder.header(
            "User-Agent",
            format!("RustMailer/{}", rustmailer_version!()),
        );

        if let Some(headers) = task_info {
            for (key, value) in headers {
                request_builder = request_builder.header(&key, &value);
            }
        }

        // Set headers if provided
        if let Some(headers) = headers {
            for (key, value) in headers {
                request_builder = request_builder.header(&key, &value);
            }
        }

        // Send the request with JSON payload
        let response = request_builder
            .json(payload) // Serialize the payload to JSON
            .send()
            .await
            .map_err(|e| raise_error!(format!("{:#?}", e), ErrorCode::InternalError))?;

        // Check for successful response status
        if !response.status().is_success() {
            return Err(raise_error!(
                format!("Error response: {}", response.status()),
                ErrorCode::HttpResponseError
            ));
        }

        Ok(response)
    }
}
