'use client';

import { Confidence } from '@/lib/types';

function getColor(score: number): string {
  if (score >= 0.8) return 'bg-green-500';
  if (score >= 0.5) return 'bg-amber-500';
  return 'bg-red-500';
}

function getLabel(score: number): string {
  if (score >= 0.8) return 'High';
  if (score >= 0.5) return 'Medium';
  return 'Low';
}

export function ConfidenceMeter({ confidence, compact }: { confidence: Confidence; compact?: boolean }) {
  const pct = Math.round(confidence.score * 100);
  const color = getColor(confidence.score);
  const label = getLabel(confidence.score);

  if (compact) {
    return (
      <span className="text-[10px] text-slate-500">{pct}%</span>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <div className="w-24 h-2 bg-slate-700 rounded-full overflow-hidden">
        <div className={`h-full ${color} rounded-full transition-all`} style={{ width: `${pct}%` }} />
      </div>
      <span className="text-xs text-slate-400">{pct}% {label}</span>
      {confidence.factors?.length > 0 && (
        <div className="group relative">
          <span className="text-xs text-slate-500 cursor-help">(?)</span>
          <div className="hidden group-hover:block absolute bottom-full left-0 mb-1 p-2 bg-slate-800 border border-slate-700 rounded text-xs text-slate-300 whitespace-nowrap z-10">
            {confidence.factors.map((f, i) => (
              <div key={i}>{f.replace(/_/g, ' ')}</div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
