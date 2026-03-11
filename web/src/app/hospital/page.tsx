'use client';

/**
 * Hospital Simulation Dashboard
 *
 * Real-time visualization of ClinicClaw's multi-patient simulation. Patient
 * swim lanes are built dynamically from the /v1/simulate/dynamic response —
 * no hardcoded encounter IDs. Agent execution blocks appear as SSE events
 * arrive. Each block pulses while the LLM is executing, then shows a
 * confidence score on completion.
 *
 * Layout:
 *   - Top bar:  title + SSE connection indicator + mode toggle + "Start" button
 *   - Left 60%: patient swim lanes (one per encounter, dynamic)
 *   - Right 40%: live stats panel (agents running/completed/failed, events/sec)
 */

import { useState, useEffect, useRef, useMemo, useCallback } from 'react';
import Link from 'next/link';
import {
  Activity,
  Play,
  Loader2,
  CheckCircle2,
  XCircle,
  Radio,
  BarChart3,
  Zap,
  Box,
  Clock,
  Shield,
  ShieldCheck,
  ShieldAlert,
  Users,
  Stethoscope,
  Timer,
} from 'lucide-react';
import { useAllEventsStream } from '@/hooks/use-all-events-stream';
import type { AgentEvent } from '@/lib/agent-events';

// -- Patient lane (dynamic) ---------------------------------------------------

interface PatientLane {
  encounterId: string;
  name: string;
  condition: string;
  agents: string[];
  color: string;
}

const LANE_COLORS = [
  'from-blue-950/60 to-slate-900',
  'from-violet-950/60 to-slate-900',
  'from-emerald-950/60 to-slate-900',
  'from-orange-950/60 to-slate-900',
  'from-teal-950/60 to-slate-900',
  'from-rose-950/60 to-slate-900',
  'from-cyan-950/60 to-slate-900',
  'from-pink-950/60 to-slate-900',
  'from-amber-950/60 to-slate-900',
  'from-indigo-950/60 to-slate-900',
  'from-lime-950/60 to-slate-900',
  'from-fuchsia-950/60 to-slate-900',
  'from-sky-950/60 to-slate-900',
];

// -- Agent display config -----------------------------------------------------

interface AgentConfig {
  abbr: string;
  label: string;
  style: string;
  activeStyle: string;
  doneStyle: string;
  failedStyle: string;
}

const AGENT_CONFIG: Record<string, AgentConfig> = {
  triage_assess: {
    abbr: 'TR',
    label: 'Triage',
    style: 'bg-blue-900/70 border-blue-600 text-blue-300',
    activeStyle: 'bg-blue-800 border-blue-400 text-blue-200 shadow-blue-500/30 shadow-md',
    doneStyle: 'bg-blue-950 border-blue-700 text-blue-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  triage: {
    abbr: 'TR',
    label: 'Triage',
    style: 'bg-blue-900/70 border-blue-600 text-blue-300',
    activeStyle: 'bg-blue-800 border-blue-400 text-blue-200 shadow-blue-500/30 shadow-md',
    doneStyle: 'bg-blue-950 border-blue-700 text-blue-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  nurse_assess: {
    abbr: 'NA',
    label: 'Nurse',
    style: 'bg-purple-900/70 border-purple-600 text-purple-300',
    activeStyle: 'bg-purple-800 border-purple-400 text-purple-200 shadow-purple-500/30 shadow-md',
    doneStyle: 'bg-purple-950 border-purple-700 text-purple-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  ambient_doc: {
    abbr: 'AD',
    label: 'Ambient Doc',
    style: 'bg-emerald-900/70 border-emerald-600 text-emerald-300',
    activeStyle: 'bg-emerald-800 border-emerald-400 text-emerald-200 shadow-emerald-500/30 shadow-md',
    doneStyle: 'bg-emerald-950 border-emerald-700 text-emerald-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  order_entry: {
    abbr: 'OE',
    label: 'Order Entry',
    style: 'bg-orange-900/70 border-orange-600 text-orange-300',
    activeStyle: 'bg-orange-800 border-orange-400 text-orange-200 shadow-orange-500/30 shadow-md',
    doneStyle: 'bg-orange-950 border-orange-700 text-orange-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  prior_auth: {
    abbr: 'PA',
    label: 'Prior Auth',
    style: 'bg-red-900/70 border-red-600 text-red-300',
    activeStyle: 'bg-red-800 border-red-400 text-red-200 shadow-red-500/30 shadow-md',
    doneStyle: 'bg-red-950 border-red-700 text-red-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  lab_review: {
    abbr: 'LR',
    label: 'Lab Review',
    style: 'bg-cyan-900/70 border-cyan-600 text-cyan-300',
    activeStyle: 'bg-cyan-800 border-cyan-400 text-cyan-200 shadow-cyan-500/30 shadow-md',
    doneStyle: 'bg-cyan-950 border-cyan-700 text-cyan-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  discharge_plan: {
    abbr: 'DC',
    label: 'Discharge',
    style: 'bg-teal-900/70 border-teal-600 text-teal-300',
    activeStyle: 'bg-teal-800 border-teal-400 text-teal-200 shadow-teal-500/30 shadow-md',
    doneStyle: 'bg-teal-950 border-teal-700 text-teal-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  discharge: {
    abbr: 'DC',
    label: 'Discharge',
    style: 'bg-teal-900/70 border-teal-600 text-teal-300',
    activeStyle: 'bg-teal-800 border-teal-400 text-teal-200 shadow-teal-500/30 shadow-md',
    doneStyle: 'bg-teal-950 border-teal-700 text-teal-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  pharmacy_review: {
    abbr: 'PH',
    label: 'Pharmacy',
    style: 'bg-pink-900/70 border-pink-600 text-pink-300',
    activeStyle: 'bg-pink-800 border-pink-400 text-pink-200 shadow-pink-500/30 shadow-md',
    doneStyle: 'bg-pink-950 border-pink-700 text-pink-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
  pharmacy: {
    abbr: 'PH',
    label: 'Pharmacy',
    style: 'bg-pink-900/70 border-pink-600 text-pink-300',
    activeStyle: 'bg-pink-800 border-pink-400 text-pink-200 shadow-pink-500/30 shadow-md',
    doneStyle: 'bg-pink-950 border-pink-700 text-pink-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  },
};

function getAgentConfig(agentName: string): AgentConfig {
  return AGENT_CONFIG[agentName] ?? {
    abbr: (agentName ?? '??').slice(0, 2).toUpperCase(),
    label: agentName,
    style: 'bg-slate-800 border-slate-600 text-slate-300',
    activeStyle: 'bg-slate-700 border-slate-500 text-slate-200 shadow-md',
    doneStyle: 'bg-slate-900 border-slate-700 text-slate-400',
    failedStyle: 'bg-red-950 border-red-700 text-red-400',
  };
}

// -- Agent execution state ----------------------------------------------------

type AgentStatus = 'running' | 'completed' | 'failed';

interface AgentExecution {
  key: string;
  agentName: string;
  encounterId: string;
  status: AgentStatus;
  confidence: number | null;
  elapsedMs: number | null;
  llmActive: boolean;
  startedAt: string;
}

// -- Event processing ---------------------------------------------------------

function buildExecutionMap(events: AgentEvent[]): Map<string, AgentExecution[]> {
  const execs = new Map<string, AgentExecution>();
  const laneOrder = new Map<string, string[]>();

  for (const event of events) {
    const { encounter_id, agent_name, event_type } = event;

    if (event_type.kind === 'agent_started') {
      const key = event.id;
      const exec: AgentExecution = {
        key,
        agentName: agent_name,
        encounterId: encounter_id,
        status: 'running',
        confidence: null,
        elapsedMs: null,
        llmActive: false,
        startedAt: event.timestamp,
      };
      execs.set(key, exec);
      const lane = laneOrder.get(encounter_id) ?? [];
      lane.push(key);
      laneOrder.set(encounter_id, lane);
      continue;
    }

    const laneKeys = laneOrder.get(encounter_id);
    if (!laneKeys) continue;

    let targetKey: string | null = null;
    for (let i = laneKeys.length - 1; i >= 0; i--) {
      const k = laneKeys[i];
      const e = execs.get(k);
      if (e && e.agentName === agent_name) {
        targetKey = k;
        break;
      }
    }
    if (!targetKey) continue;

    const exec = execs.get(targetKey)!;

    switch (event_type.kind) {
      case 'llm_call':
        exec.llmActive = event_type.status === 'started';
        break;
      case 'agent_completed':
        exec.status = 'completed';
        exec.confidence = event_type.confidence_score;
        exec.elapsedMs = event_type.elapsed_ms;
        exec.llmActive = false;
        break;
      case 'agent_failed':
        exec.status = 'failed';
        exec.llmActive = false;
        break;
    }
  }

  const result = new Map<string, AgentExecution[]>();
  for (const [encounterId, keys] of laneOrder) {
    result.set(encounterId, keys.map((k) => execs.get(k)!));
  }
  return result;
}

// -- Sub-components -----------------------------------------------------------

function AgentBlock({ exec }: { exec: AgentExecution }) {
  const cfg = getAgentConfig(exec.agentName);

  const baseClasses =
    'relative flex flex-col items-center justify-center min-w-[3.5rem] h-14 px-2 rounded-lg border text-xs font-bold transition-all duration-300 animate-slide-in select-none';

  let styleClasses: string;
  if (exec.status === 'failed') {
    styleClasses = cfg.failedStyle;
  } else if (exec.status === 'completed') {
    styleClasses = cfg.doneStyle;
  } else if (exec.llmActive) {
    styleClasses = cfg.activeStyle + ' animate-clinical-pulse';
  } else {
    styleClasses = cfg.style;
  }

  return (
    <div
      className={`${baseClasses} ${styleClasses}`}
      title={`${cfg.label} - ${exec.status}${exec.confidence != null ? ` - ${(exec.confidence * 100).toFixed(0)}% confidence` : ''}${exec.elapsedMs != null ? ` - ${(exec.elapsedMs / 1000).toFixed(1)}s` : ''}`}
    >
      <span className="text-sm font-black tracking-wider leading-none">
        {cfg.abbr}
      </span>
      <div className="mt-1 flex items-center justify-center h-3.5">
        {exec.status === 'running' && exec.llmActive && (
          <Loader2 className="w-3 h-3 animate-spin opacity-90" />
        )}
        {exec.status === 'running' && !exec.llmActive && (
          <div className="w-1.5 h-1.5 rounded-full bg-current opacity-60 animate-pulse" />
        )}
        {exec.status === 'completed' && exec.confidence != null && (
          <span className="font-clinical-mono text-[9px] opacity-80 tabular-nums">
            {(exec.confidence * 100).toFixed(0)}%
          </span>
        )}
        {exec.status === 'completed' && exec.confidence == null && (
          <CheckCircle2 className="w-3 h-3 opacity-70" />
        )}
        {exec.status === 'failed' && (
          <XCircle className="w-3 h-3 opacity-70" />
        )}
      </div>
    </div>
  );
}

function SwimLane({
  lane,
  executions,
}: {
  lane: PatientLane;
  executions: AgentExecution[];
}) {
  const hasActivity = executions.length > 0;
  const isActive = executions.some((e) => e.status === 'running');
  const completedCount = executions.filter((e) => e.status === 'completed').length;
  const totalAgents = lane.agents.length;

  return (
    <div
      className={`flex items-stretch rounded-xl border transition-all duration-500 bg-gradient-to-r ${lane.color} ${
        isActive
          ? 'border-slate-600 shadow-lg shadow-black/30'
          : 'border-slate-800'
      }`}
    >
      {/* Patient label column */}
      <div className="flex-shrink-0 w-52 px-4 py-3 flex flex-col justify-center border-r border-slate-800/60">
        <div className="flex items-center gap-2">
          <div
            className={`w-2 h-2 rounded-full flex-shrink-0 transition-colors duration-300 ${
              isActive
                ? 'bg-emerald-400 animate-pulse'
                : hasActivity
                ? 'bg-slate-600'
                : 'bg-slate-700'
            }`}
          />
          <span className="text-slate-200 text-xs font-semibold leading-tight truncate">
            {lane.name}
          </span>
        </div>
        <div className="flex items-center gap-2 mt-1 pl-4">
          <span className="text-slate-500 text-[10px]">{lane.condition}</span>
          {hasActivity && (
            <>
              <span className="text-slate-700 text-[10px]">|</span>
              <span className="text-slate-600 text-[10px] font-clinical-mono">
                {completedCount}/{totalAgents}
              </span>
            </>
          )}
        </div>
      </div>

      {/* Agent block flow area */}
      <div className="flex-1 flex items-center gap-2.5 px-4 py-3 overflow-x-auto">
        {executions.length === 0 ? (
          <span className="text-slate-700 text-xs italic">
            awaiting agents...
          </span>
        ) : (
          executions.map((exec) => (
            <AgentBlock key={exec.key} exec={exec} />
          ))
        )}
      </div>
    </div>
  );
}

function StatsPanel({
  events,
  executions,
  eventsPerSec,
  lanes,
  elapsedSec,
}: {
  events: AgentEvent[];
  executions: AgentExecution[];
  eventsPerSec: number;
  lanes: PatientLane[];
  elapsedSec: number;
}) {
  const running = executions.filter((e) => e.status === 'running').length;
  const completed = executions.filter((e) => e.status === 'completed').length;
  const failed = executions.filter((e) => e.status === 'failed').length;
  const llmActive = executions.filter((e) => e.llmActive).length;

  const avgConfidence = useMemo(() => {
    const done = executions.filter(
      (e) => e.status === 'completed' && e.confidence != null
    );
    if (done.length === 0) return null;
    const sum = done.reduce((acc, e) => acc + (e.confidence ?? 0), 0);
    return sum / done.length;
  }, [executions]);

  const totalTurns = events.filter(
    (e) => e.event_type.kind === 'turn_creation'
  ).length;

  const fhirWrites = events.filter(
    (e) => e.event_type.kind === 'fhir_write'
  ).length;

  const policyAllows = events.filter((e) => {
    if (e.event_type.kind !== 'policy_evaluation') return false;
    return e.event_type.decision === 'Allow';
  }).length;

  const policyDenials = events.filter((e) => {
    if (e.event_type.kind !== 'policy_evaluation') return false;
    return e.event_type.decision === 'Deny';
  }).length;

  const policyApprovals = events.filter((e) => {
    if (e.event_type.kind !== 'policy_evaluation') return false;
    return e.event_type.decision === 'RequireApproval';
  }).length;

  return (
    <div className="flex flex-col gap-4">
      {/* Simulation overview */}
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <Users className="w-4 h-4 text-slate-400" />
          <span className="text-slate-300 text-sm font-semibold">
            Simulation
          </span>
          {elapsedSec > 0 && (
            <span className="ml-auto text-slate-600 text-xs font-clinical-mono flex items-center gap-1">
              <Timer className="w-3 h-3" />
              {Math.floor(elapsedSec / 60)}:{String(Math.floor(elapsedSec % 60)).padStart(2, '0')}
            </span>
          )}
        </div>
        <div className="grid grid-cols-2 gap-3 mb-3">
          <div className="rounded-lg border px-3 py-2 bg-slate-800/50 border-slate-700">
            <div className="text-slate-200 font-clinical-mono text-xl font-black tabular-nums">
              {lanes.length}
            </div>
            <div className="text-slate-500 text-xs">Patients</div>
          </div>
          <div className="rounded-lg border px-3 py-2 bg-slate-800/50 border-slate-700">
            <div className="text-slate-200 font-clinical-mono text-xl font-black tabular-nums">
              {lanes.reduce((a, l) => a + l.agents.length, 0)}
            </div>
            <div className="text-slate-500 text-xs">Total Agents</div>
          </div>
        </div>
      </div>

      {/* Agent status counters */}
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <Stethoscope className="w-4 h-4 text-slate-400" />
          <span className="text-slate-300 text-sm font-semibold">
            Agent Status
          </span>
        </div>
        <div className="grid grid-cols-3 gap-3">
          <StatBox
            label="Running"
            value={running}
            color="text-blue-400"
            bgColor="bg-blue-950/50 border-blue-800"
          />
          <StatBox
            label="Completed"
            value={completed}
            color="text-emerald-400"
            bgColor="bg-emerald-950/50 border-emerald-800"
          />
          <StatBox
            label="Failed"
            value={failed}
            color="text-red-400"
            bgColor="bg-red-950/50 border-red-800"
          />
        </div>
        {llmActive > 0 && (
          <div className="mt-3 flex items-center gap-2 text-xs text-blue-400">
            <Loader2 className="w-3 h-3 animate-spin" />
            <span>
              {llmActive} LLM call{llmActive !== 1 ? 's' : ''} in flight
            </span>
          </div>
        )}
      </div>

      {/* VERITAS policy decisions */}
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <Shield className="w-4 h-4 text-slate-400" />
          <span className="text-slate-300 text-sm font-semibold">
            VERITAS Policy
          </span>
        </div>
        <div className="space-y-2.5">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
              <span className="text-slate-400 text-xs">Allowed</span>
            </div>
            <span className="text-emerald-400 text-xs font-clinical-mono font-bold tabular-nums">
              {policyAllows}
            </span>
          </div>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <ShieldAlert className="w-3.5 h-3.5 text-amber-500" />
              <span className="text-slate-400 text-xs">Require Approval</span>
            </div>
            <span className="text-amber-400 text-xs font-clinical-mono font-bold tabular-nums">
              {policyApprovals}
            </span>
          </div>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <XCircle className="w-3.5 h-3.5 text-red-500" />
              <span className="text-slate-400 text-xs">Denied</span>
            </div>
            <span className={`text-xs font-clinical-mono font-bold tabular-nums ${policyDenials > 0 ? 'text-red-400' : 'text-slate-600'}`}>
              {policyDenials}
            </span>
          </div>
        </div>
      </div>

      {/* Throughput metrics */}
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
        <div className="flex items-center gap-2 mb-4">
          <Zap className="w-4 h-4 text-slate-400" />
          <span className="text-slate-300 text-sm font-semibold">
            Throughput
          </span>
        </div>
        <div className="space-y-3">
          <MetricRow label="Events / sec" value={eventsPerSec.toFixed(1)} mono />
          <MetricRow label="Total events" value={events.length.toLocaleString()} mono />
          <MetricRow label="Turns created" value={totalTurns.toString()} mono />
          <MetricRow label="FHIR writes" value={fhirWrites.toString()} mono />
          {avgConfidence != null && (
            <MetricRow
              label="Avg confidence"
              value={`${(avgConfidence * 100).toFixed(1)}%`}
              mono
            />
          )}
        </div>
      </div>

      {/* Agent legend */}
      <div className="bg-slate-900 border border-slate-800 rounded-xl p-5">
        <p className="text-slate-400 text-xs font-semibold uppercase tracking-wider mb-3">
          Agent Legend
        </p>
        <div className="grid grid-cols-2 gap-1.5">
          {Object.entries(AGENT_CONFIG)
            .filter(([key]) => !['triage', 'discharge', 'pharmacy'].includes(key))
            .map(([, cfg]) => (
            <div key={cfg.abbr + cfg.label} className="flex items-center gap-2">
              <span
                className={`inline-flex items-center justify-center w-7 h-5 rounded text-[10px] font-black border ${cfg.doneStyle}`}
              >
                {cfg.abbr}
              </span>
              <span className="text-slate-500 text-xs">{cfg.label}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function StatBox({
  label,
  value,
  color,
  bgColor,
}: {
  label: string;
  value: number;
  color: string;
  bgColor: string;
}) {
  return (
    <div className={`rounded-lg border px-3 py-2.5 text-center ${bgColor}`}>
      <div className={`font-clinical-mono text-2xl font-black tabular-nums ${color}`}>
        {value}
      </div>
      <div className="text-slate-500 text-xs mt-0.5">{label}</div>
    </div>
  );
}

function MetricRow({
  label,
  value,
  mono,
  alert,
}: {
  label: string;
  value: string;
  mono?: boolean;
  alert?: boolean;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-slate-500 text-xs">{label}</span>
      <span
        className={`text-xs tabular-nums ${
          alert ? 'text-amber-400 font-semibold' : 'text-slate-300'
        } ${mono ? 'font-clinical-mono' : ''}`}
      >
        {value}
      </span>
    </div>
  );
}

// -- Page ---------------------------------------------------------------------

export default function HospitalSimulationPage() {
  const [simRunning, setSimRunning] = useState(false);
  const [simError, setSimError] = useState<string | null>(null);
  const [lanes, setLanes] = useState<PatientLane[]>([]);
  const [simStartTime, setSimStartTime] = useState<number | null>(null);
  const [elapsedSec, setElapsedSec] = useState(0);
  const [maxPathways, setMaxPathways] = useState(5);

  const { events, connected, clearEvents } = useAllEventsStream({
    maxEvents: 2000,
    enabled: true,
  });

  // Elapsed timer
  useEffect(() => {
    if (!simStartTime) return;
    const interval = setInterval(() => {
      setElapsedSec((Date.now() - simStartTime) / 1000);
    }, 1000);
    return () => clearInterval(interval);
  }, [simStartTime]);

  // Events-per-second (3s sliding window)
  const [eventsPerSec, setEventsPerSec] = useState(0);
  const eventCountRef = useRef(0);
  const prevEventLengthRef = useRef(0);

  useEffect(() => {
    const delta = events.length - prevEventLengthRef.current;
    prevEventLengthRef.current = events.length;
    eventCountRef.current += delta;
  }, [events.length]);

  useEffect(() => {
    const interval = setInterval(() => {
      setEventsPerSec(eventCountRef.current / 3);
      eventCountRef.current = 0;
    }, 3000);
    return () => clearInterval(interval);
  }, []);

  // Build execution map
  const executionMap = useMemo(() => buildExecutionMap(events), [events]);

  const allExecutions = useMemo(() => {
    const flat: AgentExecution[] = [];
    for (const execs of executionMap.values()) {
      flat.push(...execs);
    }
    return flat;
  }, [executionMap]);

  // Auto-stop when all agents complete
  useEffect(() => {
    if (!simRunning || lanes.length === 0 || allExecutions.length === 0) return;
    const totalExpected = lanes.reduce((a, l) => a + l.agents.length, 0);
    const totalDone = allExecutions.filter(
      (e) => e.status === 'completed' || e.status === 'failed'
    ).length;
    if (totalDone >= totalExpected) {
      setSimRunning(false);
    }
  }, [simRunning, lanes, allExecutions]);

  const handleStartSimulation = useCallback(async () => {
    setSimError(null);
    setSimRunning(true);
    setLanes([]);
    clearEvents();
    setSimStartTime(Date.now());
    setElapsedSec(0);

    try {
      const res = await fetch('/api/v1/simulate/dynamic', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'Authorization': 'Bearer dev-token',
        },
        body: JSON.stringify({ speed: 'fast', max_pathways: maxPathways }),
      });

      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        throw new Error(body.error || `HTTP ${res.status}`);
      }

      const data = await res.json();

      // Build dynamic lanes from response
      const dynamicLanes: PatientLane[] = (data.patients || []).map(
        (p: { encounter_id: string; patient_display: string; conditions: string[]; agents: string[] }, i: number) => ({
          encounterId: p.encounter_id,
          name: p.patient_display || `Patient ${i + 1}`,
          condition: (p.conditions || [])[0]?.replace(/\s*\(.*?\)/g, '') || 'Unknown',
          agents: p.agents || [],
          color: LANE_COLORS[i % LANE_COLORS.length],
        })
      );

      setLanes(dynamicLanes);
    } catch (err) {
      setSimError(err instanceof Error ? err.message : 'Simulation failed');
      setSimRunning(false);
    }
  }, [clearEvents, maxPathways]);

  return (
    <div className="flex flex-col h-full bg-slate-950 overflow-hidden">
      {/* Top bar */}
      <div className="flex-shrink-0 flex items-center justify-between px-6 py-3 border-b border-slate-800 bg-slate-900">
        <div className="flex items-center gap-3">
          <Activity className="w-5 h-5 text-blue-400" strokeWidth={2} />
          <div>
            <h1 className="text-white font-bold text-sm tracking-wide">
              ClinicClaw Hospital Simulation
            </h1>
            <p className="text-slate-500 text-xs mt-0.5">
              {lanes.length > 0
                ? `${lanes.length} patient pathways · ${lanes.reduce((a, l) => a + l.agents.length, 0)} agent executions · VERITAS trust layer`
                : 'Dynamic pathways · Synthea FHIR patients · Real-time SSE'}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-4">
          {/* SSE connection */}
          <div className="flex items-center gap-2">
            <Radio
              className={`w-3.5 h-3.5 transition-colors ${
                connected ? 'text-emerald-400' : 'text-slate-600'
              }`}
            />
            <span
              className={`text-xs transition-colors ${
                connected ? 'text-emerald-400' : 'text-slate-500'
              }`}
            >
              {connected ? 'SSE connected' : 'Connecting...'}
            </span>
          </div>

          {/* Pathway count selector */}
          <div className="flex items-center gap-2">
            <span className="text-slate-500 text-xs">Patients:</span>
            <select
              value={maxPathways}
              onChange={(e) => setMaxPathways(Number(e.target.value))}
              disabled={simRunning}
              className="bg-slate-800 text-slate-300 text-xs rounded-lg border border-slate-700 px-2 py-1.5 focus:outline-none focus:border-blue-600 disabled:opacity-50"
            >
              {[1, 2, 3, 5, 8, 10, 13].map((n) => (
                <option key={n} value={n}>{n}</option>
              ))}
            </select>
          </div>

          {/* 3D link */}
          <Link
            href="/hospital/3d"
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-slate-800 text-slate-400 hover:text-white hover:bg-slate-700 transition-colors"
          >
            <Box className="w-3.5 h-3.5" />
            3D View
          </Link>

          {/* Start button */}
          <button
            onClick={handleStartSimulation}
            disabled={simRunning || !connected}
            className={`flex items-center gap-2 px-5 py-2.5 rounded-lg text-sm font-semibold transition-all duration-200 ${
              simRunning || !connected
                ? 'bg-slate-800 text-slate-500 cursor-not-allowed'
                : 'bg-blue-600 hover:bg-blue-500 text-white shadow-lg shadow-blue-900/40 hover:shadow-blue-800/50'
            }`}
          >
            {simRunning ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Running...
              </>
            ) : (
              <>
                <Play className="w-4 h-4" />
                Start Simulation
              </>
            )}
          </button>
        </div>
      </div>

      {/* Error banner */}
      {simError && (
        <div className="flex-shrink-0 px-6 py-2 bg-red-950 border-b border-red-800 text-red-300 text-sm">
          <strong>Error:</strong> {simError}
        </div>
      )}

      {/* Main layout */}
      <div className="flex-1 flex overflow-hidden">
        {/* Patient swim lanes (60%) */}
        <div className="flex-[3] flex flex-col gap-2.5 p-4 overflow-y-auto clinical-scroll border-r border-slate-800">
          <div className="flex items-center justify-between mb-1">
            <span className="text-slate-500 text-xs font-semibold uppercase tracking-wider">
              Patient Swim Lanes
            </span>
            <span className="text-slate-600 text-xs font-clinical-mono">
              {allExecutions.length} agent executions
            </span>
          </div>

          {lanes.length > 0 ? (
            lanes.map((lane) => (
              <SwimLane
                key={lane.encounterId}
                lane={lane}
                executions={executionMap.get(lane.encounterId) ?? []}
              />
            ))
          ) : (
            /* Pre-simulation state */
            <div className="flex-1 flex items-center justify-center">
              <div className="text-center max-w-md">
                <Activity className="w-12 h-12 text-slate-700 mx-auto mb-4" />
                <p className="text-slate-400 text-sm font-medium mb-2">
                  Ready to simulate
                </p>
                <p className="text-slate-600 text-xs leading-relaxed">
                  Select the number of patients and press{' '}
                  <span className="text-blue-400 font-semibold">
                    Start Simulation
                  </span>{' '}
                  to run concurrent patient pathways with real Synthea FHIR data
                  and local Ollama LLM inference.
                </p>
                <p className="text-slate-700 text-xs mt-3">
                  Each patient is routed through appropriate AI agents based on
                  their clinical conditions. Every action is policy-gated,
                  audited, and verifiable.
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Stats panel (40%) */}
        <div className="flex-[2] p-4 overflow-y-auto clinical-scroll">
          <div className="flex items-center justify-between mb-3">
            <span className="text-slate-500 text-xs font-semibold uppercase tracking-wider">
              Live Stats
            </span>
            {events.length > 0 && (
              <button
                onClick={() => {
                  clearEvents();
                  setLanes([]);
                  setSimStartTime(null);
                  setElapsedSec(0);
                }}
                className="text-slate-600 hover:text-slate-400 text-xs transition-colors"
              >
                Clear
              </button>
            )}
          </div>
          <StatsPanel
            events={events}
            executions={allExecutions}
            eventsPerSec={eventsPerSec}
            lanes={lanes}
            elapsedSec={elapsedSec}
          />
        </div>
      </div>
    </div>
  );
}
