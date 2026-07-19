import type { ModelTarget } from '@/types/metrics'

export function ModelCatalog({ targets }: { targets: ModelTarget[] }) {
  if (targets.length === 0) return null

  return (
    <section className="shrink-0 border-b border-white/[0.04] bg-[#0a0a0d]/80 px-3 py-1.5" aria-label="Served model estates">
      <div className="flex gap-1.5 overflow-x-auto">
        {targets.map((target) => {
          const names = target.models.length > 0 ? target.models : target.aliases
          return (
            <div key={`${target.estate}-${target.slot_id}`} className="min-w-52 max-w-80 rounded-md border border-white/[0.05] bg-white/[0.02] px-2 py-1">
              <div className="flex items-center gap-1.5 text-[10px] uppercase tracking-wider">
                <span className={`size-1.5 rounded-full ${target.up ? 'bg-[#76B900]' : 'bg-amber-500'}`} />
                <span className="font-semibold text-zinc-300">{target.estate}</span>
                <span className="truncate text-zinc-600">{target.hardware}</span>
              </div>
              <div className="truncate font-mono text-xs text-zinc-100" title={names.join(', ')}>
                {names.join(' · ') || target.slot_id}
              </div>
              {target.models.length > 0 && target.aliases.length > 0 && (
                <div className="truncate font-mono text-[9px] text-zinc-600" title={target.aliases.join(', ')}>
                  {target.aliases.join(' · ')}
                </div>
              )}
            </div>
          )
        })}
      </div>
    </section>
  )
}
