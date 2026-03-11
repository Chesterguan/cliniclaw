'use client';

import { useState } from 'react';
import useSWR from 'swr';
import { getFeedbackStats } from '@/lib/api';
import {
  BarChart3,
  Activity,
  ShieldCheck,
  AlertTriangle,
  TrendingUp,
  RefreshCw,
  ChevronRight,
} from 'lucide-react';

export default function AdminPage() {
  const [selectedAgent, setSelectedAgent] = useState<string | undefined>();
  const { data: stats, isLoading } = useSWR(
    ['feedback-stats', selectedAgent],
    () => getFeedbackStats(selectedAgent),
    { refreshInterval: 10000 }
  );

  const agents = [
    { key: 'ambient_doc', label: 'Ambient Doc', abbr: 'AD' },
    { key: 'order_entry', label: 'Order Entry', abbr: 'OE' },
    { key: 'prior_auth', label: 'Prior Auth', abbr: 'PA' },
  ];

  const resolved = stats ? stats.total_turns - stats.pending : 0;
  const acceptRate = resolved > 0 ? (stats!.accepted / resolved) * 100 : 0;
  const modifyRate = resolved > 0 ? (stats!.modified / resolved) * 100 : 0;
  const rejectRate = resolved > 0 ? (stats!.rejected / resolved) * 100 : 0;

  return (
    <div className="max-w-5xl mx-auto px-6 py-8">
      {/* Header */}
      <div className="flex items-center justify-between mb-8">
        <div>
          <div className="flex items-center gap-2.5 mb-1">
            <BarChart3 className="w-5 h-5 text-slate-400" />
            <h1 className="text-lg font-semibold text-slate-900 tracking-tight">
              Agent Performance
            </h1>
          </div>
          <p className="text-xs text-slate-400 ml-[30px]">
            Accuracy, feedback patterns, and confidence calibration
          </p>
        </div>
        <button
          onClick={() => window.location.reload()}
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-slate-500 bg-white border border-slate-200 rounded-lg hover:bg-slate-50 transition-colors"
        >
          <RefreshCw className="w-3 h-3" />
          Refresh
        </button>
      </div>

      {/* Agent filter tabs */}
      <div className="flex items-center gap-1.5 mb-6">
        <button
          onClick={() => setSelectedAgent(undefined)}
          className={`px-3 py-1.5 text-xs rounded-lg transition-colors ${
            !selectedAgent
              ? 'bg-slate-900 text-white'
              : 'bg-white text-slate-500 border border-slate-200 hover:bg-slate-50 hover:text-slate-700'
          }`}
        >
          All Agents
        </button>
        {agents.map(a => (
          <button
            key={a.key}
            onClick={() => setSelectedAgent(a.key)}
            className={`px-3 py-1.5 text-xs rounded-lg transition-colors ${
              selectedAgent === a.key
                ? 'bg-slate-900 text-white'
                : 'bg-white text-slate-500 border border-slate-200 hover:bg-slate-50 hover:text-slate-700'
            }`}
          >
            {a.label}
          </button>
        ))}
      </div>

      {isLoading && !stats ? (
        <div className="text-center py-16 text-slate-400 text-sm">
          Loading statistics...
        </div>
      ) : stats ? (
        <div className="space-y-5">

          {/* ── Stat cards ──────────────────────────────────────────────── */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <StatCard
              label="Total Turns"
              value={stats.total_turns}
              icon={<Activity className="w-3.5 h-3.5" />}
            />
            <StatCard
              label="Pending Review"
              value={stats.pending}
              icon={<AlertTriangle className="w-3.5 h-3.5" />}
              accent={stats.pending > 0 ? 'amber' : undefined}
            />
            <StatCard
              label="Accepted"
              value={stats.accepted}
              icon={<ShieldCheck className="w-3.5 h-3.5" />}
              accent={stats.accepted > 0 ? 'green' : undefined}
            />
            <StatCard
              label="Escalated"
              value={stats.escalated}
              icon={<TrendingUp className="w-3.5 h-3.5" />}
              accent={stats.escalated > 0 ? 'cyan' : undefined}
            />
          </div>

          {/* ── Resolution breakdown ────────────────────────────────────── */}
          <div className="bg-white border border-slate-200 rounded-xl p-5">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-medium text-slate-800">
                Resolution Breakdown
              </h2>
              <span className="text-xs text-slate-400 font-mono-data">
                {resolved} resolved
              </span>
            </div>
            <div className="space-y-3">
              <BreakdownRow
                label="Accepted"
                value={stats.accepted}
                pct={acceptRate}
                color="bg-emerald-500"
                trackColor="bg-emerald-50"
              />
              <BreakdownRow
                label="Modified"
                value={stats.modified}
                pct={modifyRate}
                color="bg-sky-500"
                trackColor="bg-sky-50"
              />
              <BreakdownRow
                label="Rejected"
                value={stats.rejected}
                pct={rejectRate}
                color="bg-rose-400"
                trackColor="bg-rose-50"
              />
            </div>
          </div>

          {/* ── Confidence calibration ──────────────────────────────────── */}
          <div className="bg-white border border-slate-200 rounded-xl p-5">
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-medium text-slate-800">
                Confidence Calibration
              </h2>
              <ConfidenceLabel score={stats.avg_confidence} />
            </div>

            {/* Gauge bar */}
            <div className="mb-3">
              <div className="h-2 bg-slate-100 rounded-full overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all duration-500 ${confidenceBarColor(stats.avg_confidence)}`}
                  style={{ width: `${Math.round(stats.avg_confidence * 100)}%` }}
                />
              </div>
              <div className="flex justify-between mt-1.5">
                <span className="text-[10px] text-slate-300">0%</span>
                <span className="text-[10px] text-slate-300">50%</span>
                <span className="text-[10px] text-slate-300">100%</span>
              </div>
            </div>

            <p className="text-xs text-slate-400 leading-relaxed">
              High acceptance + low confidence → raise threshold.
              High rejection + high confidence → model needs recalibration.
            </p>
          </div>

          {/* ── Quick stats row ─────────────────────────────────────────── */}
          <div className="grid grid-cols-2 gap-3">
            <div className="bg-white border border-slate-200 rounded-xl p-4 flex items-center justify-between">
              <div>
                <div className="text-xs text-slate-400 mb-0.5">Rejected</div>
                <div className="text-lg font-semibold text-slate-800 font-mono-data">
                  {stats.rejected}
                </div>
              </div>
              {stats.rejected > 0 && (
                <div className="w-8 h-8 rounded-full bg-rose-50 flex items-center justify-center">
                  <AlertTriangle className="w-3.5 h-3.5 text-rose-400" />
                </div>
              )}
            </div>
            <div className="bg-white border border-slate-200 rounded-xl p-4 flex items-center justify-between">
              <div>
                <div className="text-xs text-slate-400 mb-0.5">Modified</div>
                <div className="text-lg font-semibold text-slate-800 font-mono-data">
                  {stats.modified}
                </div>
              </div>
              {stats.modified > 0 && (
                <div className="w-8 h-8 rounded-full bg-sky-50 flex items-center justify-center">
                  <ChevronRight className="w-3.5 h-3.5 text-sky-400" />
                </div>
              )}
            </div>
          </div>

          {/* ── Mock data banner ────────────────────────────────────────── */}
          <div className="bg-amber-50/80 border border-amber-200/60 rounded-xl px-4 py-3">
            <span className="text-xs">
              <strong className="text-amber-700">MOCK DATA</strong>
              <span className="text-amber-600/80">
                {' '}— Feedback statistics reflect synthetic agent executions.
                Real turn resolution requires clinician review via the chart workflow.
              </span>
            </span>
          </div>
        </div>
      ) : null}
    </div>
  );
}

/* ── StatCard ───────────────────────────────────────────────────────────── */

function StatCard({
  label,
  value,
  icon,
  accent,
}: {
  label: string;
  value: number;
  icon: React.ReactNode;
  accent?: 'green' | 'amber' | 'cyan';
}) {
  const accentStyles = {
    green:  { dot: 'bg-emerald-400', text: 'text-emerald-600', bg: 'bg-emerald-50' },
    amber:  { dot: 'bg-amber-400',   text: 'text-amber-600',   bg: 'bg-amber-50' },
    cyan:   { dot: 'bg-cyan-400',    text: 'text-cyan-600',    bg: 'bg-cyan-50' },
  };

  const style = accent ? accentStyles[accent] : null;

  return (
    <div className="bg-white border border-slate-200 rounded-xl p-4">
      <div className="flex items-center gap-1.5 mb-2">
        <span className="text-slate-300">{icon}</span>
        <span className="text-xs text-slate-400">{label}</span>
      </div>
      <div className="flex items-center gap-2">
        <span
          className={`text-2xl font-semibold font-mono-data tracking-tight ${
            style ? style.text : 'text-slate-800'
          }`}
        >
          {value}
        </span>
        {style && value > 0 && (
          <span className={`w-1.5 h-1.5 rounded-full ${style.dot}`} />
        )}
      </div>
    </div>
  );
}

/* ── BreakdownRow ───────────────────────────────────────────────────────── */

function BreakdownRow({
  label,
  value,
  pct,
  color,
  trackColor,
}: {
  label: string;
  value: number;
  pct: number;
  color: string;
  trackColor: string;
}) {
  const displayPct = isNaN(pct) ? 0 : pct;
  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-slate-500 w-16">{label}</span>
      <div className={`flex-1 h-2 ${trackColor} rounded-full overflow-hidden`}>
        <div
          className={`h-full ${color} rounded-full transition-all duration-500`}
          style={{ width: `${Math.max(displayPct, 0)}%` }}
        />
      </div>
      <span className="text-xs text-slate-500 font-mono-data w-16 text-right">
        {displayPct.toFixed(0)}%
        <span className="text-slate-300 ml-1">({value})</span>
      </span>
    </div>
  );
}

/* ── Confidence helpers ─────────────────────────────────────────────────── */

function confidenceBarColor(score: number): string {
  if (score >= 0.8) return 'bg-emerald-400';
  if (score >= 0.5) return 'bg-amber-400';
  return 'bg-rose-400';
}

function ConfidenceLabel({ score }: { score: number }) {
  const pct = Math.round(score * 100);
  const labelText = score >= 0.8 ? 'High' : score >= 0.5 ? 'Medium' : 'Low';
  const colorClass =
    score >= 0.8
      ? 'text-emerald-600 bg-emerald-50 border-emerald-200'
      : score >= 0.5
        ? 'text-amber-600 bg-amber-50 border-amber-200'
        : 'text-rose-600 bg-rose-50 border-rose-200';

  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 text-[11px] font-medium rounded-md border ${colorClass}`}
    >
      <span className="font-mono-data">{pct}%</span>
      <span>{labelText}</span>
    </span>
  );
}
