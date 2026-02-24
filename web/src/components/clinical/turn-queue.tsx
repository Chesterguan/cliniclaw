'use client';

import { useMemo } from 'react';
import useSWR from 'swr';
import { listTurns } from '@/lib/api';
import { TurnCard } from './turn-card';
import { ChainView } from './chain-view';
import type { Turn } from '@/lib/types';

/** Group turns into chains. A chain is rooted at a turn with no trigger. */
function groupChains(turns: Turn[]): { chains: Turn[][]; standalone: Turn[] } {
  const byId = new Map(turns.map(t => [t.id, t]));
  const visited = new Set<string>();
  const chains: Turn[][] = [];
  const standalone: Turn[] = [];

  // Find chain roots (triggered turns whose trigger is also in this list)
  for (const turn of turns) {
    if (visited.has(turn.id)) continue;

    // Walk up to find root
    let root = turn;
    while (root.triggered_by_turn_id && byId.has(root.triggered_by_turn_id)) {
      root = byId.get(root.triggered_by_turn_id)!;
    }

    // Walk down from root collecting chain
    const chain: Turn[] = [];
    const queue = [root.id];
    while (queue.length > 0) {
      const tid = queue.shift()!;
      if (visited.has(tid)) continue;
      visited.add(tid);
      const t = byId.get(tid);
      if (t) {
        chain.push(t);
        // Find children
        for (const child of turns) {
          if (child.triggered_by_turn_id === tid && !visited.has(child.id)) {
            queue.push(child.id);
          }
        }
      }
    }

    if (chain.length > 1) {
      chains.push(chain);
    } else if (chain.length === 1) {
      standalone.push(chain[0]);
    }
  }

  return { chains, standalone };
}

export function TurnQueue({ workspaceId }: { workspaceId: string }) {
  const { data: turns, mutate } = useSWR(
    workspaceId ? `turns-${workspaceId}` : null,
    () => listTurns(workspaceId),
    { refreshInterval: 5000 }
  );

  const { chains, standalone } = useMemo(() => {
    if (!turns || turns.length === 0) return { chains: [], standalone: [] };
    return groupChains(turns);
  }, [turns]);

  if (!turns || turns.length === 0) {
    return (
      <div className="text-center py-8 text-slate-500">
        No turns yet. Agent proposals will appear here for review.
      </div>
    );
  }

  const allTurns = [...(chains.flat()), ...standalone];
  const pending = allTurns.filter(t => t.status === 'pending');
  const resolved = allTurns.filter(t => t.status !== 'pending');

  return (
    <div className="space-y-4">
      {/* Chain visualizations */}
      {chains.map((chain, i) => (
        <ChainView key={`chain-${i}`} chain={chain} />
      ))}

      {pending.length > 0 && (
        <div>
          <h3 className="text-sm font-medium text-amber-400 mb-2">
            Pending Review ({pending.length})
          </h3>
          <div className="space-y-3">
            {pending.map(t => (
              <TurnCard key={t.id} turn={t} onResolved={() => mutate()} />
            ))}
          </div>
        </div>
      )}

      {resolved.length > 0 && (
        <div>
          <h3 className="text-sm font-medium text-slate-400 mb-2 mt-6">
            Resolved ({resolved.length})
          </h3>
          <div className="space-y-3">
            {resolved.map(t => (
              <TurnCard key={t.id} turn={t} />
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
