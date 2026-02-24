'use client';

import { useState } from 'react';
import useSWR from 'swr';
import { getFeedbackStats } from '@/lib/api';
import { ConfidenceMeter } from '@/components/clinical/confidence-meter';

export default function AdminPage() {
  const [selectedAgent, setSelectedAgent] = useState<string | undefined>();
  const { data: stats } = useSWR(
    ['feedback-stats', selectedAgent],
    () => getFeedbackStats(selectedAgent),
    { refreshInterval: 10000 }
  );

  const agents = ['ambient_doc', 'order_entry', 'prior_auth'];
  const agentLabels: Record<string, string> = {
    ambient_doc: 'Ambient Documentation',
    order_entry: 'Order Entry',
    prior_auth: 'Prior Authorization',
  };

  const resolved = stats ? stats.total_turns - stats.pending : 0;
  const acceptRate = resolved > 0 ? (stats!.accepted / resolved) * 100 : 0;
  const modifyRate = resolved > 0 ? (stats!.modified / resolved) * 100 : 0;
  const rejectRate = resolved > 0 ? (stats!.rejected / resolved) * 100 : 0;

  return (
    <div className="max-w-5xl mx-auto p-6">
      <h1 className="text-2xl font-bold text-slate-100 mb-1">Agent Performance</h1>
      <p className="text-sm text-slate-400 mb-6">
        Monitor agent accuracy, review feedback patterns, and calibrate confidence thresholds.
      </p>

      {/* Agent filter */}
      <div className="flex gap-2 mb-6">
        <button
          onClick={() => setSelectedAgent(undefined)}
          className={`px-3 py-1.5 text-sm rounded ${!selectedAgent ? 'bg-slate-600 text-white' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'}`}
        >
          All Agents
        </button>
        {agents.map(a => (
          <button
            key={a}
            onClick={() => setSelectedAgent(a)}
            className={`px-3 py-1.5 text-sm rounded ${selectedAgent === a ? 'bg-slate-600 text-white' : 'bg-slate-800 text-slate-400 hover:bg-slate-700'}`}
          >
            {agentLabels[a] || a}
          </button>
        ))}
      </div>

      {stats ? (
        <div className="space-y-6">
          {/* Overview cards */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <StatCard label="Total Turns" value={stats.total_turns} />
            <StatCard label="Pending" value={stats.pending} color="amber" />
            <StatCard label="Accepted" value={stats.accepted} color="green" />
            <StatCard label="Modified" value={stats.modified} color="blue" />
          </div>

          {/* Accuracy breakdown */}
          <div className="bg-slate-800/50 rounded-lg p-4">
            <h2 className="text-sm font-medium text-slate-300 mb-3">Resolution Breakdown</h2>
            <div className="space-y-2">
              <Bar label="Accepted" pct={acceptRate} color="bg-green-500" />
              <Bar label="Modified" pct={modifyRate} color="bg-blue-500" />
              <Bar label="Rejected" pct={rejectRate} color="bg-red-500" />
            </div>
          </div>

          {/* Confidence calibration */}
          <div className="bg-slate-800/50 rounded-lg p-4">
            <h2 className="text-sm font-medium text-slate-300 mb-3">Confidence Calibration</h2>
            <div className="flex items-center gap-4">
              <ConfidenceMeter confidence={{ score: stats.avg_confidence, factors: ['average_across_all_turns'] }} />
              <span className="text-sm text-slate-400">
                Average confidence: {(stats.avg_confidence * 100).toFixed(1)}%
              </span>
            </div>
            <p className="text-xs text-slate-500 mt-2">
              If acceptance rate is high but confidence is low, the threshold can be raised.
              If rejection rate is high and confidence is high, the model needs recalibration.
            </p>
          </div>

          {/* Summary stats */}
          <div className="bg-slate-800/50 rounded-lg p-4">
            <h2 className="text-sm font-medium text-slate-300 mb-3">Quick Stats</h2>
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div>
                <span className="text-slate-500">Escalated:</span>{' '}
                <span className="text-orange-400">{stats.escalated}</span>
              </div>
              <div>
                <span className="text-slate-500">Rejected:</span>{' '}
                <span className="text-red-400">{stats.rejected}</span>
              </div>
            </div>
          </div>
        </div>
      ) : (
        <div className="text-center py-12 text-slate-500">Loading statistics...</div>
      )}
    </div>
  );
}

function StatCard({ label, value, color }: { label: string; value: number; color?: string }) {
  const textColor = color === 'green' ? 'text-green-400' : color === 'blue' ? 'text-blue-400' : color === 'amber' ? 'text-amber-400' : 'text-slate-200';
  return (
    <div className="bg-slate-800/50 rounded-lg p-4">
      <div className="text-xs text-slate-500 mb-1">{label}</div>
      <div className={`text-2xl font-bold ${textColor}`}>{value}</div>
    </div>
  );
}

function Bar({ label, pct, color }: { label: string; pct: number; color: string }) {
  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-slate-400 w-16">{label}</span>
      <div className="flex-1 h-3 bg-slate-700 rounded-full overflow-hidden">
        <div className={`h-full ${color} rounded-full transition-all`} style={{ width: `${Math.max(pct, 0)}%` }} />
      </div>
      <span className="text-xs text-slate-400 w-12 text-right">{isNaN(pct) ? '0' : pct.toFixed(0)}%</span>
    </div>
  );
}
