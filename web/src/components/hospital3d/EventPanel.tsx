'use client';

import { useRef, useEffect, useState } from 'react';
import type { AgentEvent } from '@/lib/agent-events';
import { eventKindLabels } from '@/lib/agent-events';
import { AGENT_AVATARS } from '@/lib/hospital3d/constants';

interface EventPanelProps {
  events: AgentEvent[];
}

export function EventPanel({ events }: EventPanelProps) {
  const [collapsed, setCollapsed] = useState(false);
  const scrollRef = useRef<HTMLDivElement>(null);

  // Auto-scroll to bottom on new events
  useEffect(() => {
    if (scrollRef.current && !collapsed) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [events.length, collapsed]);

  const recent = events.slice(-30);

  if (collapsed) {
    return (
      <button
        onClick={() => setCollapsed(false)}
        className="absolute bottom-4 right-4 z-10 px-2.5 py-1 text-[10px] font-mono tracking-widest text-white/30 hover:text-white/60 bg-black/50 transition-colors"
      >
        [{events.length}]
      </button>
    );
  }

  return (
    <div className="absolute bottom-4 right-4 z-10 w-72 max-h-64 bg-black/60 overflow-hidden flex flex-col">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-1.5 border-b border-white/5">
        <span className="text-white/30 text-[9px] font-mono tracking-widest uppercase">
          Event Log
        </span>
        <button
          onClick={() => setCollapsed(true)}
          className="text-white/20 hover:text-white/50 text-xs font-mono transition-colors leading-none"
          aria-label="Collapse event log"
        >
          ×
        </button>
      </div>

      {/* Event list — monospace terminal log */}
      <div ref={scrollRef} className="flex-1 overflow-y-auto px-3 py-1 clinical-scroll">
        {recent.map((event, i) => {
          const config = AGENT_AVATARS[event.agent_name];
          const label = eventKindLabels[event.event_type.kind] ?? event.event_type.kind;
          const isImportant =
            event.event_type.kind === 'agent_started' ||
            event.event_type.kind === 'agent_completed' ||
            event.event_type.kind === 'agent_failed' ||
            event.event_type.kind === 'llm_call';

          // Color: cyan for completions, red for failures, muted slate for everything else
          const textColor =
            event.event_type.kind === 'agent_failed'
              ? 'text-red-400/80'
              : event.event_type.kind === 'agent_completed'
                ? 'text-cyan-400/80'
                : 'text-white/30';

          return (
            <div
              key={`${event.id}-${i}`}
              className={`flex items-baseline gap-1.5 py-px font-mono ${
                isImportant ? 'opacity-100' : 'opacity-40'
              }`}
            >
              {/* Mission control bracket tag — no colored dots */}
              <span className="text-white/20 text-[9px] flex-shrink-0">
                [{config?.abbr ?? '??'}]
              </span>
              <span className={`text-[9px] truncate ${textColor}`}>
                {label}
                {event.event_type.kind === 'agent_completed' &&
                  ` ${((event.event_type as { confidence_score: number }).confidence_score * 100).toFixed(0)}%`}
              </span>
            </div>
          );
        })}
        {events.length === 0 && (
          <p className="text-white/15 text-[9px] font-mono py-4 text-center tracking-wide">
            awaiting events...
          </p>
        )}
      </div>
    </div>
  );
}
