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

/// Parse a comma-separated estate allowlist (`"spark, coredump"`). An empty or
/// whitespace-only value yields an empty list, which means "no filtering".
pub fn parse_estates(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|estate| estate.trim())
        .filter(|estate| !estate.is_empty())
        .map(|estate| estate.to_ascii_lowercase())
        .collect()
}

/// Drop targets whose estate is not in `allowed`. An empty allowlist is a
/// no-op so the default deployment keeps showing every estate.
fn apply_estate_filter(targets: &mut Vec<ModelTarget>, allowed: &[String]) {
    if allowed.is_empty() {
        return;
    }
    targets.retain(|target| allowed.contains(&target.estate.to_ascii_lowercase()));
}

pub async fn collector_loop(
    url: String,
    estates: Vec<String>,
    shared: Arc<RwLock<Vec<ModelTarget>>>,
) {
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
                        apply_estate_filter(&mut targets, &estates);
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

    fn target(estate: &str) -> ModelTarget {
        ModelTarget {
            slot_id: format!("{estate}-slot"),
            hardware: "test".into(),
            estate: estate.into(),
            models: vec![],
            state: "up".into(),
            up: true,
            aliases: vec![],
            api_base: "http://example.invalid/v1".into(),
        }
    }

    #[test]
    fn parses_a_comma_separated_estate_list() {
        assert_eq!(parse_estates("spark"), ["spark"]);
        assert_eq!(parse_estates("spark, coredump"), ["spark", "coredump"]);
        assert_eq!(parse_estates("Spark,CLOUD"), ["spark", "cloud"]);
    }

    #[test]
    fn an_empty_estate_list_means_no_filtering() {
        assert!(parse_estates("").is_empty());
        assert!(parse_estates("  ,  ").is_empty());

        let mut targets = vec![target("spark"), target("blackwell")];
        apply_estate_filter(&mut targets, &[]);
        assert_eq!(targets.len(), 2, "empty allowlist must not drop anything");
    }

    #[test]
    fn filters_out_estates_not_on_the_allowlist() {
        let mut targets = vec![target("spark"), target("blackwell"), target("coredump")];
        apply_estate_filter(&mut targets, &parse_estates("spark"));
        assert_eq!(
            targets
                .iter()
                .map(|t| t.estate.as_str())
                .collect::<Vec<_>>(),
            ["spark"]
        );
    }

    #[test]
    fn estate_matching_ignores_case() {
        let mut targets = vec![target("Spark"), target("blackwell")];
        apply_estate_filter(&mut targets, &parse_estates("spark"));
        assert_eq!(targets.len(), 1);
    }

    #[test]
    fn an_allowlist_matching_nothing_yields_an_empty_catalog() {
        let mut targets = vec![target("spark")];
        apply_estate_filter(&mut targets, &parse_estates("nonexistent"));
        assert!(targets.is_empty());
    }

    #[test]
    fn missing_targets_is_an_empty_catalog() {
        let response: CatalogResponse =
            serde_json::from_value(serde_json::json!({})).expect("valid empty catalog");
        assert!(response.targets.is_empty());
    }
}
