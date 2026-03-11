'use client';

import { useEffect, useRef, useState, useCallback } from 'react';
import type { AgentEvent } from '@/lib/agent-events';

// SSE needs direct backend access — Next.js rewrites buffer streams
const API_BASE = process.env.NEXT_PUBLIC_API_URL || '/api';
const DEFAULT_MAX_EVENTS = 200;

interface UseEventStreamOptions {
  encounterId: string | null;
  maxEvents?: number;
}

interface UseEventStreamResult {
  events: AgentEvent[];
  connected: boolean;
  clearEvents: () => void;
}

export function useEventStream({
  encounterId,
  maxEvents = DEFAULT_MAX_EVENTS,
}: UseEventStreamOptions): UseEventStreamResult {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const esRef = useRef<EventSource | null>(null);
  const maxEventsRef = useRef(maxEvents);
  maxEventsRef.current = maxEvents;

  const clearEvents = useCallback(() => {
    setEvents([]);
  }, []);

  useEffect(() => {
    if (!encounterId) return;

    const url = `${API_BASE}/v1/events?encounter_id=${encodeURIComponent(encounterId)}`;
    const es = new EventSource(url);
    esRef.current = es;

    es.onopen = () => {
      setConnected(true);
    };

    es.onmessage = (e) => {
      try {
        const event: AgentEvent = JSON.parse(e.data);
        setEvents((prev) => {
          const next = [...prev, event];
          return next.length > maxEventsRef.current ? next.slice(-maxEventsRef.current) : next;
        });
      } catch (err) {
        console.warn('[SSE] malformed event:', err);
      }
    };

    es.onerror = () => {
      setConnected(false);
      // EventSource auto-reconnects
    };

    return () => {
      es.close();
      esRef.current = null;
      setConnected(false);
    };
  }, [encounterId]);

  return { events, connected, clearEvents };
}
