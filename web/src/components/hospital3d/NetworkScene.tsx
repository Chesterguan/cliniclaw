'use client';

/**
 * NetworkScene — network graph alternative to HospitalScene.
 *
 * Same external props interface as HospitalScene: receives AgentEvent[] from
 * the SSE stream and a CameraPreset. Internally manages its own connection
 * state via useRef (no zustand, no React state updates per event) so the
 * scene avoids unnecessary re-renders.
 *
 * Layout:
 *   Outer ring (radius 9, Y=0.5)  — 8 agent nodes, evenly spaced
 *   Inner ring (radius 3.8, Y=0.5) — 6 patient nodes, evenly spaced
 *   Both rings offset by -π/2 so the first node sits at the top (north).
 *
 * Data flow:
 *   events → eventToConnectionCommand → local connectionMap (useRef)
 *   connectionMap → React state snapshot (useState) to trigger re-render
 *     only when the SET of active connections changes (add/remove),
 *     NOT on every state transition inside an existing connection.
 *   Per-connection state transitions update the map in place and propagate
 *   to children via props derived from map values.
 *
 * Connection map key: "agentName:encounterId"
 * Value: { agentName, encounterId, state, confidence, startedAt }
 */

import { useEffect, useRef, useState, useMemo } from 'react';
import type { AgentEvent } from '@/lib/agent-events';
import { eventToConnectionCommand } from '@/lib/hospital3d/event-bridge';
import {
  AGENTS,
  PATIENTS,
  type AgentConfig,
  type PatientConfig,
} from '@/lib/hospital3d/constants';
import { Lighting } from './Lighting';
import { CameraRig, type CameraPreset } from './CameraRig';
import { GroundPlane } from './GroundPlane';
import { AmbientParticles } from './AmbientParticles';
import { AgentNode, type AgentNodeState } from './AgentNode';
import { PatientNode } from './PatientNode';
import { ConnectionBeam } from './ConnectionBeam';

// ── Props ────────────────────────────────────────────────────────────────────

interface NetworkSceneProps {
  events: AgentEvent[];
  cameraPreset: CameraPreset;
}

// ── Connection map entry ──────────────────────────────────────────────────────

interface ConnectionEntry {
  agentName: string;
  encounterId: string;
  /** AvatarState string from the event bridge. */
  state: string;
  confidence: number | null;
  /** Seconds timestamp (performance.now()/1000) when this connection was established. */
  startedAt: number;
}

// ── Ring layout ───────────────────────────────────────────────────────────────

const AGENT_RING_RADIUS   = 9;
const PATIENT_RING_RADIUS = 3.8;
const RING_Y              = 0.5;

/**
 * Compute evenly-spaced positions around a horizontal ring.
 * Starting angle is -π/2 so index 0 sits at the "north" (negative Z axis).
 */
function ringPositions(count: number, radius: number): [number, number, number][] {
  return Array.from({ length: count }, (_, i) => {
    const angle = -Math.PI / 2 + (i / count) * Math.PI * 2;
    return [
      Math.cos(angle) * radius,
      RING_Y,
      Math.sin(angle) * radius,
    ] as [number, number, number];
  });
}

// ── Ordered agent array (matches AGENTS record, deterministic order) ──────────

// AGENTS is a plain object — extract in definition order, which is stable.
const AGENT_LIST: AgentConfig[] = Object.values(AGENTS);

// Pre-compute fixed ring positions — never changes at runtime
const AGENT_POSITIONS  = ringPositions(AGENT_LIST.length, AGENT_RING_RADIUS);
const PATIENT_POSITIONS = ringPositions(PATIENTS.length, PATIENT_RING_RADIUS);

// Quick lookup: agentName → ring position index
const AGENT_INDEX: Record<string, number> = Object.fromEntries(
  AGENT_LIST.map((a, i) => [a.agentName, i]),
);

// Quick lookup: encounterId → patient ring index
const PATIENT_INDEX: Record<string, number> = Object.fromEntries(
  PATIENTS.map((p, i) => [p.encounterId, i]),
);

// ── AvatarState → AgentNodeState mapping ─────────────────────────────────────

function avatarStateToNodeState(avatarState: string): AgentNodeState {
  switch (avatarState) {
    case 'thinking':                    return 'thinking';
    case 'completed':                   return 'completed';
    case 'failed':                      return 'failed';
    case 'working': case 'writing':
    case 'walking': case 'arriving':    return 'active';
    default:                            return 'inactive';
  }
}

// ── Terminal states that should fade and eventually be removed ────────────────

const TERMINAL_STATES = new Set(['completed', 'failed']);
// How long to keep a terminal connection alive (for fade animation) before
// removing it from the map. Must be >= ConnectionBeam FADE_DURATION (2.5s).
const TERMINAL_LINGER_MS = 3000;

// ── Component ─────────────────────────────────────────────────────────────────

export function NetworkScene({ events, cameraPreset }: NetworkSceneProps) {
  // ── Connection state (no zustand — local to this component tree) ──────────

  // Map: "agentName:encounterId" → ConnectionEntry
  // We keep this in a ref so useFrame reads in children never need React state.
  // We only trigger a React re-render (via setVersion) when the MAP KEYS change
  // (connection added or removed), not on every state transition within a key.
  const connectionMapRef = useRef<Map<string, ConnectionEntry>>(new Map());

  // Incrementing counter: bumped whenever the set of active connections changes.
  // React components read connectionMapRef.current directly; this version number
  // just tells React when to re-render.
  const [version, setVersion] = useState(0);

  // How many events have already been processed (prevents re-processing on
  // every render — same pattern as HospitalScene).
  const processedCountRef = useRef(0);

  // ── Event bridging ────────────────────────────────────────────────────────

  useEffect(() => {
    const newEvents = events.slice(processedCountRef.current);
    let structureChanged = false;

    for (const event of newEvents) {
      const cmd = eventToConnectionCommand(event);
      if (!cmd) continue;

      const { agentName, encounterId, state, confidence } = cmd;
      const key = `${agentName}:${encounterId}`;
      const map = connectionMapRef.current;
      const now = performance.now() / 1000;

      if (state === 'walking') {
        // New connection: agent started working on this encounter
        const entry: ConnectionEntry = {
          agentName,
          encounterId,
          state: 'walking',
          confidence: null,
          startedAt: now,
        };
        map.set(key, entry);
        structureChanged = true;

      } else if (TERMINAL_STATES.has(state)) {
        // Terminal: update state in-place (beam fades on its own via startedAt),
        // then schedule removal after linger period.
        const existing = map.get(key);
        if (existing) {
          map.set(key, {
            ...existing,
            state,
            confidence: confidence ?? existing.confidence,
            startedAt: now,  // reset so the fade timer is accurate
          });
          // Force re-render so children receive updated state prop immediately
          setVersion(v => v + 1);

          // Schedule removal so the beam has time to fully fade out
          setTimeout(() => {
            connectionMapRef.current.delete(key);
            setVersion(v => v + 1);
          }, TERMINAL_LINGER_MS);
        }

      } else {
        // Intermediate state transition: update in place, no structural change
        const existing = map.get(key);
        if (existing) {
          map.set(key, {
            ...existing,
            state,
            confidence: confidence ?? existing.confidence,
          });
          // Trigger re-render so AgentNode/ConnectionBeam receive updated props
          setVersion(v => v + 1);
        }
      }
    }

    processedCountRef.current = events.length;

    if (structureChanged) {
      setVersion(v => v + 1);
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events.length, events]);

  // ── Derived per-node state ────────────────────────────────────────────────

  // Read the current connection map (version dependency causes re-derivation)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const connections = useMemo(() => Array.from(connectionMapRef.current.values()), [version]);

  // AgentNode state: 'inactive' unless at least one connection is active
  // When multiple connections exist for the same agent, we pick the "highest"
  // priority state (thinking > active > inactive).
  const agentNodeStates = useMemo((): Map<string, AgentNodeState> => {
    const result = new Map<string, AgentNodeState>();
    for (const conn of connections) {
      const ns = avatarStateToNodeState(conn.state);
      const prev = result.get(conn.agentName);
      // Priority: thinking > completed > failed > active > inactive
      const priority: Record<AgentNodeState, number> = {
        thinking: 5, completed: 4, failed: 3, active: 2, inactive: 1,
      };
      if (!prev || priority[ns] > priority[prev]) {
        result.set(conn.agentName, ns);
      }
    }
    return result;
  }, [connections]);

  // PatientNode activity: how many active (non-terminal) connections per encounter
  const patientActiveCounts = useMemo((): Map<string, number> => {
    const counts = new Map<string, number>();
    for (const conn of connections) {
      if (!TERMINAL_STATES.has(conn.state)) {
        counts.set(conn.encounterId, (counts.get(conn.encounterId) ?? 0) + 1);
      }
    }
    return counts;
  }, [connections]);

  // Clinician review encounters (turn_creation events)
  const clinicianEncounterIds = useMemo(() => {
    const ids = new Set<string>();
    for (const event of events) {
      if (event.event_type.kind === 'turn_creation') {
        ids.add(event.encounter_id);
      }
    }
    return ids;
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events.length, events]);

  // ── JSX ───────────────────────────────────────────────────────────────────

  return (
    <>
      <Lighting />
      <CameraRig preset={cameraPreset} followTarget={null} />

      {/* Atmospheric ground with orbit rings already baked in for radius 3.8 / 9 */}
      <GroundPlane />

      {/* Floating ambient particles for depth and atmosphere */}
      <AmbientParticles />

      {/* ── Outer ring: 8 agent nodes ────────────────────────────────── */}
      {AGENT_LIST.map((agent, i) => {
        const nodeState = agentNodeStates.get(agent.agentName) ?? 'inactive';
        return (
          <AgentNode
            key={agent.agentName}
            agentName={agent.agentName}
            label={agent.label}
            abbr={agent.abbr}
            color={agent.color}
            accentColor={agent.accentColor}
            position={AGENT_POSITIONS[i]}
            state={nodeState}
          />
        );
      })}

      {/* ── Inner ring: 6 patient nodes ──────────────────────────────── */}
      {PATIENTS.map((patient: PatientConfig, i) => {
        const count = patientActiveCounts.get(patient.encounterId) ?? 0;
        return (
          <PatientNode
            key={patient.encounterId}
            name={patient.name}
            condition={patient.condition}
            color={patient.color}
            position={PATIENT_POSITIONS[i]}
            isActive={count > 0}
            activeCount={count}
            clinicianReview={clinicianEncounterIds.has(patient.encounterId)}
          />
        );
      })}

      {/* ── Active connection beams ───────────────────────────────────── */}
      {connections.map((conn) => {
        const agentIdx   = AGENT_INDEX[conn.agentName];
        const patientIdx = PATIENT_INDEX[conn.encounterId];

        // Skip if either end doesn't exist in our ring layout
        if (agentIdx === undefined || patientIdx === undefined) return null;

        const agentConfig = AGENTS[conn.agentName];
        if (!agentConfig) return null;

        const agentPos   = AGENT_POSITIONS[agentIdx];
        const patientPos = PATIENT_POSITIONS[patientIdx];

        return (
          <ConnectionBeam
            key={`${conn.agentName}:${conn.encounterId}`}
            agentPos={agentPos}
            patientPos={patientPos}
            color={agentConfig.color}
            accentColor={agentConfig.accentColor}
            state={conn.state}
            startedAt={conn.startedAt}
            confidence={conn.confidence}
          />
        );
      })}

      {/* Subtle exponential fog — same args as HospitalScene for visual parity */}
      <fog attach="fog" args={['#060610', 28, 52]} />
    </>
  );
}
