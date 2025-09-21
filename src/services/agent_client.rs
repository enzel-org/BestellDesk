// src/services/agent_client.rs
use anyhow::{Context, Result};
use serde::Deserialize;

/// Simple response shape returned by the agent.
#[derive(Debug, Deserialize)]
struct AgentResp {
    mongo_uri: String,
}

/// Fetch the MongoDB URI from the agent.
/// - `agent_host` can be "agent.example.com:8443" or a full URL.
/// - If scheme is missing, https:// is assumed.
/// Endpoint called: <agent>/v1/mongo-uri
pub async fn fetch_mongo_uri(agent_host: &str) -> Result<String> {
    let host = agent_host.trim();
    let url = if host.starts_with("http://") || host.starts_with("https://") {
        format!("{host}/v1/mongo-uri")
    } else {
        format!("http://{host}/v1/mongo-uri")
    };

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()
        .context("failed to build HTTP client")?;

    let resp = client
        .get(&url)
        .send()
        .await
        .context("request to agent failed")?
        .error_for_status()
        .context("agent returned an error status")?;

    let body: AgentResp = resp.json().await.context("invalid JSON from agent")?;
    Ok(body.mongo_uri)
}
