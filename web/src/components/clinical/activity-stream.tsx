'use client';

import { useEffect, useRef, useMemo } from 'react';
import {
  Loader2,
  CheckCircle2,
  XCircle,
  Zap,
  Radio,
  Trash2,
} from 'lucide-react';
import type { AgentEvent } from '@/lib/agent-events';
import { agentLabels, eventKindLabels } from '@/lib/agent-events';
import { GovernancePipeline } from './governance-pipeline';

function getEventIcon(event: AgentEvent) {
  const kind = event.event_type.kind;
  if (kind === 'agent_failed') return <XCircle className="w-3.5 h-3.5 text-red-400" />;
  if (kind === 'agent_completed') return <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />;
  if (kind === 'llm_call') {
    const { status } = event.event_type; // TS narrows via discriminated union
    if (status === 'started') return <Loader2 className="w-3.5 h-3.5 text-blue-400 animate-spin" />;
    return <CheckCircle2 className="w-3.5 h-3.5 text-blue-400" />;
  }
  if (kind === 'chain_trigger') return <Zap className="w-3.5 h-3.5 text-amber-400" />;
  return <CheckCircle2 className="w-3.5 h-3.5 text-slate-500" />;
}

function getEventDetail(event: AgentEvent): string | null {
  const et = event.event_type;
  switch (et.kind) {
    case 'context_building': return et.detail;
    case 'skill_lookup': return et.matched ? `Matched: ${et.skill_id}` : 'No matching skill';
    case 'role_check': return `${et.role} — ${et.allowed ? 'allowed' : 'denied'}`;
    case 'capability_check': return `${et.capability} — ${et.valid ? 'valid' : 'invalid'}`;
    case 'population_gate': return et.passed ? 'Passed' : `Blocked: ${et.reason}`;
    case 'policy_evaluation': return `${et.decision}${et.rule_name ? ` (${et.rule_name})` : ''}`;
    case 'llm_call': return et.status === 'completed' ? `${et.elapsed_ms}ms` : null;
    case 'response_parsing': return et.detail ?? null;
    case 'cds_check': return `${et.cards_count} card(s)${et.max_severity ? ` — max: ${et.max_severity}` : ''}`;
    case 'verification': return et.passed ? (et.detail ?? 'Passed') : `Failed: ${et.detail}`;
    case 'audit_creation': return et.audit_event_id.slice(0, 8) + '...';
    case 'fhir_write': return `${et.resource_type}${et.resource_id ? ` (${et.resource_id.slice(0, 8)}...)` : ''}`;
    case 'turn_creation': return `Score: ${(et.confidence_score * 100).toFixed(0)}%`;
    case 'chain_trigger': return `${et.trigger_pattern} → ${et.target_agent}`;
    case 'agent_completed': return `${et.elapsed_ms}ms — ${(et.confidence_score * 100).toFixed(0)}% confidence`;
    case 'agent_failed': return et.error;
    default: return null;
  }
}

function getTimeDelta(event: AgentEvent, prevEvent: AgentEvent | null): string | null {
  if (!prevEvent) return null;
  const dt = new Date(event.timestamp).getTime() - new Date(prevEvent.timestamp).getTime();
  if (dt < 1) return null;
  return `+${dt}ms`;
}

interface ActivityStreamProps {
  events: AgentEvent[];
  connected: boolean;
  onClear: () => void;
}

export function ActivityStream({ events, connected, onClear }: ActivityStreamProps) {
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new events
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events.length]);

  // Group events by agent execution (separated by agent_started events)
  const currentRunEvents = useMemo(() => {
    // Find the last agent_started event and return events from there
    const lastStartIdx = events.reduce((acc, e, i) =>
      e.event_type.kind === 'agent_started' ? i : acc, -1
    );
    return lastStartIdx >= 0 ? events.slice(lastStartIdx) : events;
  }, [events]);

  const currentAgent = currentRunEvents.length > 0 ? currentRunEvents[0].agent_name : null;
  const isRunning = currentRunEvents.length > 0 &&
    !currentRunEvents.some(e => e.event_type.kind === 'agent_completed' || e.event_type.kind === 'agent_failed');

  return (
    <div className="flex flex-col h-full bg-slate-950 border-l border-slate-800">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-slate-800 flex-shrink-0">
        <div className="flex items-center gap-2">
          <div className={`w-2 h-2 rounded-full ${
            connected
              ? isRunning ? 'bg-blue-500 animate-pulse' : 'bg-emerald-500'
              : 'bg-slate-600'
          }`} />
          <span className="text-xs font-medium text-slate-300">
            {isRunning
              ? `${agentLabels[currentAgent ?? ''] ?? 'Agent'} running...`
              : connected ? 'Agent Activity' : 'Connecting...'}
          </span>
        </div>
        <div className="flex items-center gap-2">
          {events.length > 0 && (
            <button
              onClick={onClear}
              className="text-slate-500 hover:text-slate-300 transition-colors"
              title="Clear events"
            >
              <Trash2 className="w-3.5 h-3.5" />
            </button>
          )}
          <Radio className={`w-3.5 h-3.5 ${connected ? 'text-emerald-500' : 'text-slate-600'}`} />
        </div>
      </div>

      {/* Governance Pipeline */}
      {currentRunEvents.length > 0 && (
        <div className="px-3 py-2 border-b border-slate-800 flex-shrink-0">
          <GovernancePipeline events={currentRunEvents} />
        </div>
      )}

      {/* Event list */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto clinical-scroll px-3 py-2">
        {events.length === 0 ? (
          <div className="text-center py-8 text-slate-600 text-xs">
            Waiting for agent activity...
          </div>
        ) : (
          <div className="space-y-0.5">
            {events.map((event, idx) => {
              const prevEvent = idx > 0 ? events[idx - 1] : null;
              const isNewAgent = event.event_type.kind === 'agent_started';
              const detail = getEventDetail(event);
              const timeDelta = getTimeDelta(event, prevEvent);

              return (
                <div key={event.id}>
                  {/* Separator for new agent executions */}
                  {isNewAgent && idx > 0 && (
                    <div className="flex items-center gap-2 my-2">
                      <div className="h-px flex-1 bg-slate-800" />
                      <span className="text-xs text-slate-600">
                        {agentLabels[event.agent_name] ?? event.agent_name}
                      </span>
                      <div className="h-px flex-1 bg-slate-800" />
                    </div>
                  )}

                  <div className={`flex items-start gap-2 py-1 px-2 rounded text-xs animate-slide-in ${
                    event.event_type.kind === 'agent_failed' ? 'bg-red-900/20' :
                    event.event_type.kind === 'agent_completed' ? 'bg-emerald-900/10' :
                    event.event_type.kind === 'chain_trigger' ? 'bg-amber-900/15' :
                    'hover:bg-slate-900'
                  }`}>
                    {getEventIcon(event)}
                    <div className="flex-1 min-w-0">
                      <span className="text-slate-300 font-medium">
                        {eventKindLabels[event.event_type.kind] ?? event.event_type.kind}
                      </span>
                      {detail && (
                        <span className="text-slate-500 ml-1.5 truncate">
                          {detail}
                        </span>
                      )}
                    </div>
                    {timeDelta && (
                      <span className="text-slate-600 tabular-nums flex-shrink-0">
                        {timeDelta}
                      </span>
                    )}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* Footer stats */}
      {events.length > 0 && (
        <div className="px-3 py-1.5 border-t border-slate-800 flex items-center justify-between text-xs text-slate-600 flex-shrink-0">
          <span>{events.length} events</span>
          {(() => {
            const completed = currentRunEvents.find(e => e.event_type.kind === 'agent_completed');
            if (!completed || completed.event_type.kind !== 'agent_completed') return null;
            return (
              <span className="text-emerald-600">
                {completed.event_type.elapsed_ms}ms total
              </span>
            );
          })()}
        </div>
      )}
    </div>
  );
}
