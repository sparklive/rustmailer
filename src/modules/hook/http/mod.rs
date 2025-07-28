// Copyright © 2025 rustmailer.com
// Licensed under RustMailer License Agreement v1.0
// Unauthorized copying, modification, or distribution is prohibited.

use crate::modules::error::code::ErrorCode;
use crate::modules::hook::entity::HttpMethod;
use crate::modules::settings::proxy::Proxy;
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
    #[cfg(test)]
    pub fn create(client: reqwest::Client) -> HttpClient {
        Self { client }
    }

    pub async fn new(use_proxy: Option<u64>) -> RustMailerResult<HttpClient> {
        let mut builder = reqwest::ClientBuilder::new()
            .user_agent(rustmailer_version!())
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(10));

        if let Some(proxy_id) = use_proxy {
            let proxy = Proxy::get(proxy_id).await?;
            let proxy_obj = reqwest::Proxy::all(&proxy.url).map_err(|e| {
                raise_error!(
                    format!(
                        "Failed to configure SOCKS5 proxy ({}): {:#?}. Please check",
                        &proxy.url, e
                    ),
                    ErrorCode::InternalError
                )
            })?;
            builder = builder
                .redirect(reqwest::redirect::Policy::none())
                .proxy(proxy_obj);
        }

        let client = builder.build().map_err(|e| {
            raise_error!(
                format!("Failed to build HTTP client: {:#?}", e),
                ErrorCode::InternalError
            )
        })?;

        Ok(Self { client })
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
        Ok(response)
    }
}
