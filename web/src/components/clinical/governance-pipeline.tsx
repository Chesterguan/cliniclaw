'use client';

import { useMemo } from 'react';
import {
  Database,
  Shield,
  Key,
  Cpu,
  CheckCircle2,
  FileText,
} from 'lucide-react';
import type { AgentEvent, GovernanceStage } from '@/lib/agent-events';
import { eventToStage } from '@/lib/agent-events';

type StageStatus = 'waiting' | 'active' | 'completed' | 'failed';

interface StageConfig {
  id: GovernanceStage;
  label: string;
  icon: React.ReactNode;
}

const STAGES: StageConfig[] = [
  { id: 'state', label: 'State', icon: <Database className="w-3.5 h-3.5" /> },
  { id: 'policy', label: 'Policy', icon: <Shield className="w-3.5 h-3.5" /> },
  { id: 'capability', label: 'Capability', icon: <Key className="w-3.5 h-3.5" /> },
  { id: 'execution', label: 'Execution', icon: <Cpu className="w-3.5 h-3.5" /> },
  { id: 'verify', label: 'Verify', icon: <CheckCircle2 className="w-3.5 h-3.5" /> },
  { id: 'audit', label: 'Audit', icon: <FileText className="w-3.5 h-3.5" /> },
];

const ORDER: GovernanceStage[] = ['state', 'policy', 'capability', 'execution', 'verify', 'audit'];

function computeStageStatuses(events: AgentEvent[]): Record<GovernanceStage, StageStatus> {
  const statuses: Record<GovernanceStage, StageStatus> = {
    state: 'waiting',
    policy: 'waiting',
    capability: 'waiting',
    execution: 'waiting',
    verify: 'waiting',
    audit: 'waiting',
  };

  if (events.length === 0) return statuses;

  // Track which stages have been reached
  let maxReachedIdx = -1;
  let failed = false;

  for (const event of events) {
    const kind = event.event_type.kind;
    if (kind === 'agent_failed') {
      failed = true;
    }
    const stage = eventToStage[kind];
    if (stage) {
      const idx = ORDER.indexOf(stage);
      if (idx > maxReachedIdx) {
        maxReachedIdx = idx;
      }
    }
  }

  // Mark all stages up to (but not including) current as completed
  for (let i = 0; i < ORDER.length; i++) {
    if (i < maxReachedIdx) {
      statuses[ORDER[i]] = 'completed';
    } else if (i === maxReachedIdx) {
      const lastEvent = events[events.length - 1];
      const isTerminal = lastEvent.event_type.kind === 'agent_completed' || lastEvent.event_type.kind === 'agent_failed';
      statuses[ORDER[i]] = failed ? 'failed' : isTerminal ? 'completed' : 'active';
    }
  }

  return statuses;
}

const statusStyles: Record<StageStatus, string> = {
  waiting: 'bg-slate-800 border-slate-700 text-slate-500',
  active: 'bg-blue-900/50 border-blue-500 text-blue-400 animate-pulse',
  completed: 'bg-emerald-900/40 border-emerald-500 text-emerald-400',
  failed: 'bg-red-900/40 border-red-500 text-red-400',
};

const arrowStyles: Record<StageStatus, string> = {
  waiting: 'bg-slate-700',
  active: 'bg-blue-500 animate-pulse',
  completed: 'bg-emerald-500',
  failed: 'bg-red-500',
};

export function GovernancePipeline({ events }: { events: AgentEvent[] }) {
  const statuses = useMemo(() => computeStageStatuses(events), [events]);

  return (
    <div className="flex items-center gap-0.5 overflow-x-auto pb-1">
      {STAGES.map((stage, idx) => {
        const status = statuses[stage.id];
        return (
          <div key={stage.id} className="flex items-center">
            {/* Node */}
            <div
              className={`flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border text-xs font-medium transition-all duration-300 ${statusStyles[status]}`}
              title={`${stage.label}: ${status}`}
            >
              {stage.icon}
              <span className="hidden sm:inline">{stage.label}</span>
            </div>
            {/* Arrow */}
            {idx < STAGES.length - 1 && (
              <div className={`w-4 h-0.5 transition-all duration-300 ${arrowStyles[statuses[ORDER[idx + 1]] === 'waiting' ? status : statuses[ORDER[idx + 1]]]}`} />
            )}
          </div>
        );
      })}
    </div>
  );
}
