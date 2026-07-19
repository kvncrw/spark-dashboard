import { describe, expect, it } from 'vitest'
import { selectNodeMetrics, selectNodeName } from './nodeSelection'
import type { MetricsSnapshot, NodeMetricsSnapshot } from '@/types/metrics'

const hardware = {
  gpu: { name: null, utilization_percent: null, temperature_celsius: null, power_watts: null, power_limit_watts: null, clock_graphics_mhz: null, clock_sm_mhz: null, clock_memory_mhz: null, fan_speed_percent: null },
  cpu: { name: null, aggregate_percent: 0, per_core: [] },
  memory: { total_bytes: 0, used_bytes: 0, available_bytes: 0, cached_bytes: 0, gpu_estimated_bytes: null, gpu_memory_total_bytes: null, gpu_memory_used_bytes: null, is_unified: true },
  disk: { name: null, read_bytes_per_sec: 0, write_bytes_per_sec: 0 },
  network: { name: null, rx_bytes_per_sec: 0, tx_bytes_per_sec: 0 },
}

const node = (name: string, state: string, cpu: number): NodeMetricsSnapshot => ({
  name,
  state,
  ...hardware,
  cpu: { ...hardware.cpu, aggregate_percent: cpu },
})

describe('node selection', () => {
  const nodes = [node('spark-stale', 'stale', 1), node('spark-live', 'reachable', 42)]

  it('defaults to the first reachable node and preserves an existing selection', () => {
    expect(selectNodeName(nodes, null)).toBe('spark-live')
    expect(selectNodeName(nodes, 'spark-stale')).toBe('spark-stale')
  })

  it('projects selected hardware without dropping cluster-wide fields', () => {
    const metrics: MetricsSnapshot = {
      timestamp_ms: 1,
      ...hardware,
      engines: [],
      gpu_events: [],
      nodes,
    }
    const selected = selectNodeMetrics(metrics, 'spark-live')
    expect(selected?.cpu.aggregate_percent).toBe(42)
    expect(selected?.nodes).toBe(nodes)
  })
})
