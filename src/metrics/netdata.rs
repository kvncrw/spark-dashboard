use super::{
    CpuMetrics, DiskMetrics, GpuMetrics, MemoryMetrics, MetricsSnapshot, NetworkMetrics,
    NodeMetricsSnapshot,
};
use crate::engines::EngineSnapshot;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, RwLock};

const CONTEXTS: &str = "system.cpu|system.ram|nvidia_smi.gpu_utilization|nvidia_smi.gpu_memory_utilization|nvidia_smi.gpu_power_draw|nvidia_smi.gpu_temperature|nvidia_smi.gpu_clock_freq|disk.io|net.net";

#[derive(Clone, Debug)]
struct NodeInfo {
    name: String,
    state: String,
    cpu_name: Option<String>,
    network_name: Option<String>,
    total_memory: u64,
}

#[derive(Clone)]
pub struct NetdataClient {
    client: Client,
    base_url: String,
    nodes: String,
}

impl NetdataClient {
    pub fn new(base_url: &str, nodes: String) -> Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder().timeout(Duration::from_secs(5)).build()?,
            base_url: base_url.trim_end_matches('/').to_owned(),
            nodes,
        })
    }

    async fn collect(&self) -> Result<Vec<NodeMetricsSnapshot>, reqwest::Error> {
        let nodes_url = format!("{}/api/v3/nodes", self.base_url);
        let data_url = format!("{}/api/v3/data", self.base_url);
        let (nodes, data) = tokio::try_join!(
            self.client
                .get(nodes_url)
                .query(&[("scope_nodes", self.nodes.as_str())])
                .send(),
            self.client
                .get(data_url)
                .query(&[
                    ("nodes", self.nodes.as_str()),
                    ("contexts", CONTEXTS),
                    ("after", "-30"),
                    ("points", "1"),
                    ("group_by", "node,context,dimension"),
                    ("format", "json2"),
                    ("options", "seconds,unaligned,absolute"),
                ])
                .send(),
        )?;

        let node_json = nodes.error_for_status()?.json::<Value>().await?;
        let data_json = data.error_for_status()?.json::<Value>().await?;
        Ok(parse_snapshot(&node_json, &data_json))
    }
}

pub async fn metrics_collector(
    tx: broadcast::Sender<String>,
    poll_interval_ms: u64,
    client: NetdataClient,
    engine_state: std::sync::Arc<RwLock<Vec<EngineSnapshot>>>,
) {
    let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));
    loop {
        interval.tick().await;
        match client.collect().await {
            Ok(nodes) if !nodes.is_empty() => {
                let timestamp_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let primary = nodes
                    .iter()
                    .find(|node| node.state == "reachable")
                    .unwrap_or(&nodes[0]);
                let snapshot = MetricsSnapshot {
                    timestamp_ms,
                    gpu: primary.gpu.clone(),
                    cpu: primary.cpu.clone(),
                    memory: primary.memory.clone(),
                    disk: primary.disk.clone(),
                    network: primary.network.clone(),
                    engines: engine_state.read().await.clone(),
                    gpu_events: Vec::new(),
                    nodes,
                };
                if let Ok(json) = serde_json::to_string(&snapshot) {
                    let _ = tx.send(json);
                }
            }
            Ok(_) => tracing::warn!("Netdata returned no matching nodes"),
            Err(error) => tracing::warn!(%error, "failed to collect Netdata metrics"),
        }
    }
}

fn parse_snapshot(nodes: &Value, data: &Value) -> Vec<NodeMetricsSnapshot> {
    let mut info = parse_nodes(nodes);
    let values = parse_values(data);
    info.sort_by(|a, b| a.name.cmp(&b.name));
    info.into_iter()
        .map(|node| build_node(node, &values))
        .collect()
}

fn parse_nodes(value: &Value) -> Vec<NodeInfo> {
    value["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let name = node["nm"].as_str()?.to_owned();
            let labels = &node["labels"];
            Some(NodeInfo {
                name,
                state: node["state"].as_str().unwrap_or("unknown").to_owned(),
                cpu_name: labels["_system_cpu_model"].as_str().map(str::to_owned),
                network_name: labels["_net_default_iface"].as_str().map(str::to_owned),
                total_memory: node["hw"]["memory"]
                    .as_str()
                    .and_then(|raw| raw.parse().ok())
                    .unwrap_or(0),
            })
        })
        .collect()
}

fn parse_values(value: &Value) -> HashMap<(String, String, String), f64> {
    let Some(names) = value
        .pointer("/view/dimensions/names")
        .and_then(Value::as_array)
    else {
        return HashMap::new();
    };
    let Some(row) = value.pointer("/result/data/0").and_then(Value::as_array) else {
        return HashMap::new();
    };
    names
        .iter()
        .enumerate()
        .filter_map(|(index, name)| {
            let mut parts = name.as_str()?.splitn(3, ',');
            let dimension = parts.next()?.to_owned();
            let node = parts.next()?.to_owned();
            let context = parts.next()?.to_owned();
            let point = row.get(index + 1)?;
            let number = point
                .as_array()
                .and_then(|parts| parts.first())
                .unwrap_or(point)
                .as_f64()?;
            Some(((node, context, dimension), number))
        })
        .collect()
}

fn build_node(
    node: NodeInfo,
    values: &HashMap<(String, String, String), f64>,
) -> NodeMetricsSnapshot {
    let node_name = node.name.clone();
    let get = |context: &str, dimension: &str| {
        values
            .get(&(node_name.clone(), context.to_owned(), dimension.to_owned()))
            .copied()
    };
    let cpu = [
        "guest_nice",
        "guest",
        "steal",
        "softirq",
        "irq",
        "user",
        "system",
        "nice",
        "iowait",
    ]
    .iter()
    .filter_map(|dimension| get("system.cpu", dimension))
    .sum::<f64>()
    .clamp(0.0, 100.0) as f32;
    let mib = 1024.0 * 1024.0;
    let free = get("system.ram", "free").unwrap_or(0.0) * mib;
    let cached = (get("system.ram", "cached").unwrap_or(0.0)
        + get("system.ram", "buffers").unwrap_or(0.0))
        * mib;
    let used = get("system.ram", "used").unwrap_or(0.0) * mib;
    let total = if node.total_memory > 0 {
        node.total_memory
    } else {
        (free + cached + used) as u64
    };
    let gpu_present = get("nvidia_smi.gpu_utilization", "gpu").is_some();

    NodeMetricsSnapshot {
        name: node_name.clone(),
        state: node.state,
        gpu: GpuMetrics {
            name: gpu_present.then(|| "NVIDIA GB10".to_owned()),
            utilization_percent: get("nvidia_smi.gpu_utilization", "gpu").map(|v| v as u32),
            temperature_celsius: get("nvidia_smi.gpu_temperature", "temperature").map(|v| v as u32),
            power_watts: get("nvidia_smi.gpu_power_draw", "power_draw"),
            power_limit_watts: None,
            clock_graphics_mhz: get("nvidia_smi.gpu_clock_freq", "graphics").map(|v| v as u32),
            clock_sm_mhz: get("nvidia_smi.gpu_clock_freq", "sm").map(|v| v as u32),
            clock_memory_mhz: get("nvidia_smi.gpu_clock_freq", "mem").map(|v| v as u32),
            fan_speed_percent: None,
        },
        cpu: CpuMetrics {
            name: node.cpu_name,
            aggregate_percent: cpu,
            per_core: Vec::new(),
        },
        memory: MemoryMetrics {
            total_bytes: total,
            display_total_bytes: super::memory::round_up_to_marketed_gib(total),
            used_bytes: used as u64,
            available_bytes: (free + cached) as u64,
            cached_bytes: cached as u64,
            gpu_estimated_bytes: None,
            gpu_memory_total_bytes: None,
            gpu_memory_used_bytes: None,
            is_unified: gpu_present,
        },
        disk: DiskMetrics {
            name: Some("All disks".to_owned()),
            read_bytes_per_sec: (get("disk.io", "reads").unwrap_or(0.0) * mib) as u64,
            write_bytes_per_sec: (get("disk.io", "writes").unwrap_or(0.0) * mib) as u64,
        },
        network: NetworkMetrics {
            name: node.network_name,
            rx_bytes_per_sec: (get("net.net", "received").unwrap_or(0.0) * 1000.0 / 8.0) as u64,
            tx_bytes_per_sec: (get("net.net", "sent").unwrap_or(0.0) * 1000.0 / 8.0) as u64,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json2_values_into_node_metrics() {
        let nodes = serde_json::json!({"nodes": [{
            "nm": "spark-test", "state": "reachable",
            "labels": {"_system_cpu_model": "GB10 CPU", "_net_default_iface": "eth0"},
            "hw": {"memory": "134217728000"}
        }]});
        let data = serde_json::json!({
            "view": {"dimensions": {"names": [
                "user,spark-test,system.cpu", "used,spark-test,system.ram",
                "free,spark-test,system.ram", "gpu,spark-test,nvidia_smi.gpu_utilization",
                "power_draw,spark-test,nvidia_smi.gpu_power_draw", "received,spark-test,net.net"
            ]}},
            "result": {"data": [[123, [25.0, 0, 1], [1000.0, 0, 1], [100.0, 0, 1], [80.0, 0, 1], [42.5, 0, 1], [8.0, 0, 1]]]}
        });
        let parsed = parse_snapshot(&nodes, &data);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "spark-test");
        assert_eq!(parsed[0].cpu.aggregate_percent, 25.0);
        assert_eq!(parsed[0].gpu.utilization_percent, Some(80));
        assert_eq!(parsed[0].gpu.power_watts, Some(42.5));
        assert_eq!(parsed[0].network.rx_bytes_per_sec, 1000);
    }

    #[test]
    fn keeps_stale_nodes_with_empty_metrics() {
        let nodes = serde_json::json!({"nodes": [{"nm": "spark-stale", "state": "stale", "labels": {}, "hw": {}}]});
        let parsed = parse_snapshot(&nodes, &serde_json::json!({}));
        assert_eq!(parsed[0].state, "stale");
        assert_eq!(parsed[0].gpu.utilization_percent, None);
    }
}
