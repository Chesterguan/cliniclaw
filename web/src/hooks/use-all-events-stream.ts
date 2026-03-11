'use client';

import { useEffect, useRef, useState, useCallback } from 'react';
import type { AgentEvent } from '@/lib/agent-events';

// SSE needs direct backend access — Next.js rewrites buffer streams
const API_BASE = process.env.NEXT_PUBLIC_API_URL || '/api';
const DEFAULT_MAX_EVENTS = 2000;

interface UseAllEventsStreamOptions {
  maxEvents?: number;
  enabled?: boolean;
}

interface UseAllEventsStreamResult {
  events: AgentEvent[];
  connected: boolean;
  clearEvents: () => void;
}

/**
 * Like useEventStream but receives ALL events across every encounter.
 *
 * Used by the hospital simulation dashboard to render swim lanes for every
 * patient concurrently. The encounter_id query param is omitted so the SSE
 * endpoint fans out every broadcast event to this subscriber.
 */
export function useAllEventsStream({
  maxEvents = DEFAULT_MAX_EVENTS,
  enabled = true,
}: UseAllEventsStreamOptions = {}): UseAllEventsStreamResult {
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [connected, setConnected] = useState(false);
  const esRef = useRef<EventSource | null>(null);
  const maxEventsRef = useRef(maxEvents);
  maxEventsRef.current = maxEvents;

  const clearEvents = useCallback(() => {
    setEvents([]);
  }, []);

  useEffect(() => {
    if (!enabled) return;

    // No encounter_id filter — receives every event on the broadcast channel
    const url = `${API_BASE}/v1/events`;
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
          return next.length > maxEventsRef.current
            ? next.slice(-maxEventsRef.current)
            : next;
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
  }, [enabled]);

  return { events, connected, clearEvents };
}
