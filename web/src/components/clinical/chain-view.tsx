'use client';

import { Turn } from '@/lib/types';
import { ConfidenceMeter } from './confidence-meter';

const agentIcons: Record<string, string> = {
  ambient_doc: 'AD',
  order_entry: 'OE',
  prior_auth: 'PA',
};

const agentLabels: Record<string, string> = {
  ambient_doc: 'Ambient Doc',
  order_entry: 'Order Entry',
  prior_auth: 'Prior Auth',
};

const statusDot: Record<string, string> = {
  pending: 'bg-amber-400',
  accepted: 'bg-green-400',
  modified: 'bg-blue-400',
  rejected: 'bg-red-400',
  escalated: 'bg-orange-400',
};

interface ChainViewProps {
  /** All turns in the chain, ordered root-first */
  chain: Turn[];
}

/**
 * Connected agent chain visualization.
 *
 * Shows agent nodes connected by arrows with trigger labels.
 * E.g. [AmbientDoc] --"lisinopril detected"--> [OrderEntry]
 */
export function ChainView({ chain }: ChainViewProps) {
  if (chain.length < 2) return null;

  return (
    <div className="mb-4 p-3 bg-slate-800/40 border border-slate-700 rounded-lg">
      <div className="text-xs font-medium text-slate-400 mb-3 flex items-center gap-1.5">
        <svg className="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <path d="M13 5l7 7-7 7M5 5l7 7-7 7" />
        </svg>
        Agent Chain ({chain.length} turns)
      </div>

      <div className="flex items-center gap-1 overflow-x-auto pb-1">
        {chain.map((turn, i) => (
          <div key={turn.id} className="flex items-center gap-1 flex-shrink-0">
            {/* Agent node */}
            <div className="flex flex-col items-center gap-1">
              <div className="w-12 h-12 rounded-lg bg-slate-700 border border-slate-600 flex flex-col items-center justify-center">
                <span className="text-xs font-bold text-slate-300">
                  {agentIcons[turn.agent_name] || turn.agent_name.charAt(0).toUpperCase()}
                </span>
                <div className={`w-1.5 h-1.5 rounded-full mt-0.5 ${statusDot[turn.status] || 'bg-slate-500'}`} />
              </div>
              <span className="text-[10px] text-slate-500 text-center max-w-16 truncate">
                {agentLabels[turn.agent_name] || turn.agent_name}
              </span>
              <ConfidenceMeter confidence={turn.confidence} compact />
            </div>

            {/* Arrow connector */}
            {i < chain.length - 1 && (
              <div className="flex flex-col items-center mx-1">
                <div className="text-[9px] text-slate-500 mb-0.5 max-w-20 text-center truncate">
                  {extractTriggerLabel(chain[i + 1])}
                </div>
                <div className="flex items-center">
                  <div className="w-8 h-px bg-slate-600" />
                  <svg className="w-2 h-2 text-slate-500 -ml-0.5" viewBox="0 0 8 8" fill="currentColor">
                    <path d="M0 0 L8 4 L0 8 Z" />
                  </svg>
                </div>
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/** Extract a short trigger label from a chained turn's output or metadata */
function extractTriggerLabel(turn: Turn): string {
  if (turn.agent_name === 'order_entry' && turn.triggered_by_turn_id) {
    return 'medication detected';
  }
  if (turn.agent_name === 'prior_auth' && turn.triggered_by_turn_id) {
    return 'auth required';
  }
  return 'triggered';
}
