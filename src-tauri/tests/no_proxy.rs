//! ABOUTME: proves the host HTTP client ignores proxy env vars for loopback calls (#73).
//! ABOUTME: own test binary because it mutates process-wide proxy env vars.
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn host_client_reaches_loopback_directly_despite_proxy_env() {
    let proxy = MockServer::start().await;
    let target = MockServer::start().await;
    Mock::given(any()).respond_with(ResponseTemplate::new(200)).mount(&proxy).await;
    Mock::given(any()).respond_with(ResponseTemplate::new(200)).mount(&target).await;
    for var in ["HTTP_PROXY", "http_proxy", "HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"] {
        std::env::set_var(var, proxy.uri());
    }
    for var in ["NO_PROXY", "no_proxy"] {
        std::env::remove_var(var);
    }

    // Control: a default client does route loopback through the proxy, so the test can fail.
    reqwest::Client::new().get(format!("{}/ping", target.uri())).send().await.unwrap();
    assert_eq!(proxy.received_requests().await.unwrap().len(), 1, "control: default client should be proxied");

    rumble_lib::http_client().get(format!("{}/ping", target.uri())).send().await.unwrap();
    assert_eq!(proxy.received_requests().await.unwrap().len(), 1, "host client went through the proxy");
    assert_eq!(target.received_requests().await.unwrap().len(), 1, "host client did not reach the target");
}
