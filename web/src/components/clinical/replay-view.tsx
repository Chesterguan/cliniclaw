'use client';

import { useState } from 'react';
import useSWR from 'swr';
import { replayTurn } from '@/lib/api';
import type { ReplayResult, DiffEntry, Confidence } from '@/lib/types';

interface ReplayViewProps {
  turnId: string;
}

const opColors: Record<string, { bg: string; text: string; label: string }> = {
  add: { bg: 'bg-green-500/10', text: 'text-green-400', label: 'Added' },
  remove: { bg: 'bg-red-500/10', text: 'text-red-400', label: 'Removed' },
  replace: { bg: 'bg-amber-500/10', text: 'text-amber-400', label: 'Changed' },
};

/**
 * Side-by-side diff viewer for replay/what-if analysis.
 *
 * Shows original output vs replay output with highlighted differences.
 */
export function ReplayView({ turnId }: ReplayViewProps) {
  const [modifiedInput, setModifiedInput] = useState('');
  const [rerunKey, setRerunKey] = useState(0);

  const { data, error, isLoading } = useSWR<ReplayResult>(
    [`replay-${turnId}`, rerunKey],
    () => {
      const body: { modified_input?: Record<string, unknown> } = {};
      if (modifiedInput.trim()) {
        try {
          body.modified_input = JSON.parse(modifiedInput);
        } catch {
          // ignore parse error, send without modified input
        }
      }
      return replayTurn(turnId, body);
    }
  );

  const handleRerun = () => {
    // Incrementing the key changes the SWR cache key, triggering a fresh fetch
    // with the current modifiedInput captured by the fetcher closure.
    // No mutate() needed — key change already triggers refetch.
    setRerunKey(k => k + 1);
  };

  if (isLoading) {
    return (
      <div className="mt-2 p-4 bg-slate-800/50 border border-slate-700 rounded-lg animate-pulse">
        <div className="h-4 bg-slate-700 rounded w-1/3 mb-3" />
        <div className="h-32 bg-slate-700 rounded" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="mt-2 p-3 bg-red-500/10 border border-red-500/30 rounded text-xs text-red-400">
        Failed to load replay: {error instanceof Error ? error.message : 'Unknown error'}
      </div>
    );
  }

  if (!data) return null;

  const diffs = data.diff ?? [];
  const hasDiff = diffs.length > 0;

  return (
    <div className="mt-2 border border-slate-700 rounded-lg overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 bg-slate-800/70 border-b border-slate-700">
        <div className="flex items-center gap-2">
          <svg className="w-4 h-4 text-slate-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M1 4v6h6M23 20v-6h-6" />
            <path d="M20.49 9A9 9 0 005.64 5.64L1 10m22 4l-4.64 4.36A9 9 0 013.51 15" />
          </svg>
          <span className="text-sm font-medium text-slate-300">Replay Analysis</span>
          <span className="text-[10px] text-slate-500">{data.agent_name}</span>
        </div>
        {hasDiff && (
          <span className="text-[10px] px-1.5 py-0.5 bg-amber-500/20 text-amber-400 rounded-full">
            {diffs.length} difference{diffs.length !== 1 ? 's' : ''}
          </span>
        )}
      </div>

      {/* Confidence comparison */}
      {data.original_confidence && data.replay_confidence && (
        <div className="flex items-center gap-4 px-4 py-2 bg-slate-800/40 border-b border-slate-700 text-xs">
          <ConfidenceDelta
            original={data.original_confidence}
            replay={data.replay_confidence}
          />
        </div>
      )}

      {/* Side-by-side panels */}
      <div className="grid grid-cols-2 divide-x divide-slate-700">
        <div className="p-3">
          <div className="text-[10px] font-medium text-slate-500 uppercase mb-2">Original</div>
          <pre className="text-xs text-slate-300 font-mono whitespace-pre-wrap max-h-48 overflow-y-auto">
            {JSON.stringify(data.original_output, null, 2)}
          </pre>
        </div>
        <div className="p-3">
          <div className="text-[10px] font-medium text-slate-500 uppercase mb-2">Replay</div>
          {data.replay_output ? (
            <pre className="text-xs text-slate-300 font-mono whitespace-pre-wrap max-h-48 overflow-y-auto">
              {JSON.stringify(data.replay_output, null, 2)}
            </pre>
          ) : (
            <span className="text-xs text-slate-500">No replay output</span>
          )}
        </div>
      </div>

      {/* Diff entries */}
      {hasDiff && (
        <div className="border-t border-slate-700 px-4 py-2">
          <div className="text-[10px] font-medium text-slate-500 uppercase mb-2">Differences</div>
          <div className="space-y-1">
            {diffs.map((d, i) => (
              <DiffRow key={i} entry={d} />
            ))}
          </div>
        </div>
      )}

      {!hasDiff && data.replay_output && (
        <div className="border-t border-slate-700 px-4 py-2 text-xs text-green-400">
          No differences — replay produced identical output
        </div>
      )}

      {/* Input modification */}
      <div className="border-t border-slate-700 px-4 py-3">
        <div className="text-[10px] font-medium text-slate-500 uppercase mb-1">What-If: Modify Input</div>
        <textarea
          className="w-full bg-slate-800 border border-slate-600 rounded p-2 text-xs text-slate-200 font-mono"
          rows={3}
          value={modifiedInput}
          onChange={(e) => setModifiedInput(e.target.value)}
          placeholder={`Paste modified input JSON, or leave empty to re-run with original input...\n${JSON.stringify(data.input_snapshot, null, 2).substring(0, 100)}...`}
        />
        <button
          onClick={handleRerun}
          className="mt-2 px-3 py-1 text-xs bg-blue-600 hover:bg-blue-500 text-white rounded transition-colors"
        >
          Re-run Agent
        </button>
      </div>
    </div>
  );
}

function ConfidenceDelta({ original, replay }: { original: Confidence; replay: Confidence }) {
  const delta = replay.score - original.score;
  const deltaPct = Math.round(delta * 100);
  const color = delta > 0 ? 'text-green-400' : delta < 0 ? 'text-red-400' : 'text-slate-400';

  return (
    <div className="flex items-center gap-4">
      <span className="text-slate-400">
        Original: <span className="text-slate-200">{Math.round(original.score * 100)}%</span>
      </span>
      <span className="text-slate-400">
        Replay: <span className="text-slate-200">{Math.round(replay.score * 100)}%</span>
      </span>
      <span className={color}>
        {delta > 0 ? '+' : ''}{deltaPct}%
      </span>
    </div>
  );
}

function DiffRow({ entry }: { entry: DiffEntry }) {
  const style = opColors[entry.op] || opColors.replace;

  return (
    <div className={`flex items-start gap-2 p-1.5 rounded ${style.bg}`}>
      <span className={`text-[10px] font-medium px-1 py-0.5 rounded ${style.text}`}>
        {style.label}
      </span>
      <span className="text-xs text-slate-400 font-mono">{entry.path}</span>
      {entry.op === 'replace' && (
        <div className="flex-1 text-xs font-mono">
          <span className="text-red-400 line-through">
            {truncate(JSON.stringify(entry.original), 60)}
          </span>
          {' → '}
          <span className="text-green-400">
            {truncate(JSON.stringify(entry.replay), 60)}
          </span>
        </div>
      )}
    </div>
  );
}

function truncate(str: string, max: number): string {
  return str.length > max ? str.slice(0, max) + '...' : str;
}
