use crate::modules::hook::http::HttpClient;

#[tokio::test]
async fn test_connect_timeout() {
    use crate::modules::hook::entity::HttpMethod;
    use serde_json::json;

    let client = HttpClient::new().unwrap();

    let url = "http://10.255.255.1:81/";
    let payload = json!({ "test": "timeout" });

    let result = client
        .send_json_request(None, HttpMethod::Post, url, &payload, None)
        .await;

    match result {
        Err(e) => {
            let err_str = e.to_string();
            println!("Caught error: {}", err_str);
        }
        Ok(_) => panic!("Expected timeout, but got successful response"),
    }
}

#[tokio::test]
async fn test_send_to_debug_any_json() {
    use crate::modules::hook::entity::HttpMethod;
    use serde_json::json;

    let client = HttpClient::new().unwrap();

    let url = "http://127.0.0.1:15630/api/v1/debug-any-json";
    let payload = json!({
        "message": "Hello, debug!",
        "timestamp": 1688888888,
        "nested": {
            "level": "info",
            "enabled": true
        }
    });

    let result = client
        .send_json_request(None, HttpMethod::Post, url, &payload, None)
        .await;

    match result {
        Ok(response) => {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            println!("Response Status: {}", status);
            println!("Response Body: {}", text);
            assert!(status.is_success(), "Expected success response");
        }
        Err(e) => panic!("Request failed: {}", e),
    }
}
