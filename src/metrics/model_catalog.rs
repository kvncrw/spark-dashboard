use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ModelTarget {
    pub slot_id: String,
    pub hardware: String,
    pub estate: String,
    #[serde(default)]
    pub models: Vec<String>,
    pub state: String,
    pub up: bool,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub api_base: String,
}

#[derive(Debug, Deserialize)]
struct CatalogResponse {
    #[serde(default)]
    targets: Vec<ModelTarget>,
}

pub async fn collector_loop(url: String, shared: Arc<RwLock<Vec<ModelTarget>>>) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();
    let mut interval = tokio::time::interval(Duration::from_secs(15));

    loop {
        interval.tick().await;
        match client.get(&url).send().await {
            Ok(response) => match response.error_for_status() {
                Ok(response) => match response.json::<CatalogResponse>().await {
                    Ok(body) => {
                        let mut targets = body.targets;
                        targets.sort_by(|a, b| {
                            a.estate
                                .cmp(&b.estate)
                                .then_with(|| a.hardware.cmp(&b.hardware))
                        });
                        *shared.write().await = targets;
                    }
                    Err(error) => tracing::warn!(%error, "invalid model catalog response"),
                },
                Err(error) => tracing::warn!(%error, "model catalog returned an error"),
            },
            Err(error) => tracing::warn!(%error, "failed to fetch model catalog"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_status_dashboard_catalog() {
        let response: CatalogResponse = serde_json::from_value(serde_json::json!({
            "targets": [{
                "slot_id": "coredump",
                "hardware": "RTX 5090",
                "estate": "coredump",
                "models": ["qwen3.6:35b-a3b"],
                "state": "up",
                "up": true,
                "aliases": ["local-qwen-coredump"],
                "api_base": "http://ollama-coredump:11434"
            }]
        }))
        .expect("valid catalog");
        assert_eq!(response.targets.len(), 1);
        assert_eq!(response.targets[0].estate, "coredump");
        assert_eq!(response.targets[0].aliases, ["local-qwen-coredump"]);
    }

    #[test]
    fn missing_targets_is_an_empty_catalog() {
        let response: CatalogResponse =
            serde_json::from_value(serde_json::json!({})).expect("valid empty catalog");
        assert!(response.targets.is_empty());
    }
}
