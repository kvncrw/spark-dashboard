import type { MetricsSnapshot, NodeMetricsSnapshot } from '@/types/metrics'

export function selectNodeName(
  nodes: NodeMetricsSnapshot[],
  current: string | null,
): string | null {
  if (nodes.length === 0) return null
  if (current && nodes.some((node) => node.name === current)) return current
  return (nodes.find((node) => node.state === 'reachable') ?? nodes[0]).name
}

export function selectNodeMetrics(
  metrics: MetricsSnapshot | null,
  selected: string | null,
): MetricsSnapshot | null {
  if (!metrics || !selected) return metrics
  const node = metrics.nodes?.find((candidate) => candidate.name === selected)
  return node ? { ...metrics, ...node } : metrics
}
