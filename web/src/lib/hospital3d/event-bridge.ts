// Maps AgentEvent stream to AvatarCommand for the 3D hospital floor simulation.
//
// State machine transitions (one-way path per agent lifecycle):
//   agent_started        → 'walking'  — avatar spawns, moves toward patient room
//   context_building     → 'working'  — arrived, gathering context
//   policy_evaluation    → 'working'  — policy gate evaluation
//   llm_call started     → 'thinking' — bright glow during LLM call
//   llm_call completed   → 'writing'  — transcribing output to FHIR
//   fhir_write           → 'writing'  — FHIR resource write
//   agent_completed      → 'completed'— flash, linger, then return to home
//   agent_failed         → 'failed'   — red flash, linger, then return to home

import type { AgentEvent } from '@/lib/agent-events';
import type { AvatarCommand } from './avatar-state-machine';

// Kept as `eventToConnectionCommand` so existing call-sites in HospitalScene
// don't need renaming — the function name is the only public API contract.
export function eventToConnectionCommand(event: AgentEvent): AvatarCommand | null {
  const { agent_name, encounter_id, event_type, timestamp } = event;
  const ts = new Date(timestamp).getTime();

  switch (event_type.kind) {
    case 'agent_started':
      return {
        agentName: agent_name,
        encounterId: encounter_id,
        state: 'walking',
        timestamp: ts,
      };

    case 'context_building':
      return {
        agentName: agent_name,
        encounterId: encounter_id,
        state: 'working',
        timestamp: ts,
      };

    case 'policy_evaluation':
      // Only emit on 'evaluating' — allow/deny decisions don't need separate
      // visual states for the floor simulation.
      if (event_type.decision === 'evaluating') {
        return {
          agentName: agent_name,
          encounterId: encounter_id,
          state: 'working',
          timestamp: ts,
        };
      }
      return null;

    case 'llm_call':
      if (event_type.status === 'started') {
        return {
          agentName: agent_name,
          encounterId: encounter_id,
          state: 'thinking',
          timestamp: ts,
        };
      }
      if (event_type.status === 'completed') {
        return {
          agentName: agent_name,
          encounterId: encounter_id,
          state: 'writing',
          timestamp: ts,
        };
      }
      return null;

    case 'fhir_write':
      return {
        agentName: agent_name,
        encounterId: encounter_id,
        state: 'writing',
        timestamp: ts,
      };

    case 'agent_completed':
      return {
        agentName: agent_name,
        encounterId: encounter_id,
        state: 'completed',
        confidence: event_type.confidence_score,
        timestamp: ts,
      };

    case 'agent_failed':
      return {
        agentName: agent_name,
        encounterId: encounter_id,
        state: 'failed',
        error: event_type.error,
        timestamp: ts,
      };

    default:
      return null;
  }
}
