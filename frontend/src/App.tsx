import { useMemo, useState } from 'react'
import { useMetrics } from './hooks/useMetrics'
import { useMetricsHistory } from './hooks/useMetricsHistory'
import { ConnectionBadge } from './components/ConnectionBadge'
import { Dashboard } from './components/views/Dashboard'
import { selectNodeMetrics, selectNodeName } from './lib/nodeSelection'
import type { GpuEvent, InferenceRequest } from './types/events'

function App() {
  const { metrics, connectionStatus, isStale } = useMetrics()
  const nodes = useMemo(() => metrics?.nodes ?? [], [metrics?.nodes])
  const [selectedNode, setSelectedNode] = useState<string | null>(null)
  const effectiveSelectedNode = selectNodeName(nodes, selectedNode)

  const displayedMetrics = useMemo(() => {
    return selectNodeMetrics(metrics, effectiveSelectedNode)
  }, [metrics, effectiveSelectedNode])

  const history = useMetricsHistory(displayedMetrics, effectiveSelectedNode)

  const { getEvents, getRequests } = history

  const events = useMemo((): GpuEvent[] =>
    getEvents().map((e) => ({
      timestamp_ms: e.timestamp_ms,
      event_type: e.event_type as GpuEvent['event_type'],
      detail: e.detail,
    })),
    [getEvents],
  )

  const requests = useMemo((): InferenceRequest[] =>
    getRequests().map((r) => ({
      start_ms: r.start_ms,
      end_ms: r.end_ms,
      tps: r.tokens_per_sec,
      ttft_ms: r.ttft_ms,
    })),
    [getRequests],
  )

  return (
    <div className="h-dvh flex flex-col bg-[#08080a] overflow-hidden">
      <header className="shrink-0 border-b border-white/[0.04] px-4 py-1.5 flex justify-between items-center gap-3">
        <h1 className="text-xl font-semibold text-zinc-100 tracking-tight" style={{ fontFamily: 'Inter, sans-serif' }}>
          <span className="text-[#76B900]">Spark</span>{' '}
          <span className="text-zinc-500 font-normal">Dashboard</span>
        </h1>
        {nodes.length > 0 && (
          <nav className="flex items-center gap-1 overflow-x-auto" aria-label="Spark node">
            {nodes.map((node) => (
              <button
                key={node.name}
                type="button"
                onClick={() => setSelectedNode(node.name)}
                className={`flex items-center gap-1.5 rounded-md px-2 py-1 text-xs font-mono transition-colors ${
                  node.name === effectiveSelectedNode
                    ? 'bg-[#76B900]/15 text-[#9bd52b] ring-1 ring-[#76B900]/30'
                    : 'text-zinc-500 hover:bg-white/[0.04] hover:text-zinc-300'
                }`}
              >
                <span className={`size-1.5 rounded-full ${node.state === 'reachable' ? 'bg-[#76B900]' : 'bg-amber-500'}`} />
                {node.name.replace(/^spark-/, '')}
              </button>
            ))}
          </nav>
        )}
        <ConnectionBadge status={connectionStatus} isStale={isStale} />
      </header>

      <main className={`flex-1 min-h-0 flex flex-col p-3 lg:p-4 2xl:p-5 min-[1920px]:p-6 ${isStale ? 'opacity-50' : ''}`}>
        {!metrics && connectionStatus !== 'connected' && (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center">
              <h2 className="text-xl font-bold text-zinc-50 mb-2">Waiting for metrics</h2>
              <p className="text-zinc-400">
                Connecting to the metrics server at {window.location.origin}. Make sure spark-dashboard is running.
              </p>
            </div>
          </div>
        )}

        <Dashboard
          metrics={displayedMetrics}
          history={history}
          events={events}
          requests={requests}
        />
      </main>
    </div>
  )
}

export default App
