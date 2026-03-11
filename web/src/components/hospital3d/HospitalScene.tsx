'use client';

/**
 * HospitalScene — 3D floor-plan composition for the ClinicClaw hospital simulation.
 *
 * Renders the physical hospital layout (rooms, hallway, floor) with avatar figures
 * that walk between their home departments and patient exam rooms in response to
 * real-time SSE agent events.
 *
 * Data flow:
 *   events (AgentEvent[]) → eventToConnectionCommand → useSceneState.processCommand
 *   useSceneState.avatars → <AgentAvatar> per active avatar instance
 *   useSceneState.avatars → activeRooms map → <HospitalFloor> room glow
 *   PATIENTS constant     → <PatientBed> × 6
 */

import { useEffect, useRef, useMemo } from 'react';
import type { AgentEvent } from '@/lib/agent-events';
import { eventToConnectionCommand } from '@/lib/hospital3d/event-bridge';
import { useSceneState } from '@/hooks/use-scene-state';
import { PATIENTS, AGENTS, type AgentConfig } from '@/lib/hospital3d/constants';
import { HospitalFloor } from './HospitalFloor';
import { PatientBed } from './PatientBed';
import { AgentAvatar } from './AgentAvatar';
import { Lighting } from './Lighting';
import { CameraRig, type CameraPreset } from './CameraRig';

interface HospitalSceneProps {
  events: AgentEvent[];
  cameraPreset: CameraPreset;
}

export function HospitalScene({ events, cameraPreset }: HospitalSceneProps) {
  // Track how many events have already been processed so we only forward new ones
  const processedCountRef = useRef(0);

  // Bridge SSE events → avatar state machine
  useEffect(() => {
    const newEvents = events.slice(processedCountRef.current);
    for (const event of newEvents) {
      const cmd = eventToConnectionCommand(event);
      if (cmd) {
        useSceneState.getState().processCommand(cmd);
      }

      // Spawn clinician review indicator when a Turn requires approval
      if (event.event_type.kind === 'turn_creation') {
        const { turn_id } = event.event_type as { turn_id: string };
        useSceneState.getState().spawnClinician(turn_id, event.encounter_id);
      }
    }
    processedCountRef.current = events.length;
  }, [events.length, events]);

  // Subscribe to avatar map for rendering — Map reference changes on every mutation
  const avatars = useSceneState((s) => s.avatars);

  // Build activeRooms map: roomId → agent accent color
  // When multiple agents are in the same room we take the last one (arbitrary);
  // the room color is a hint, not a precise readout.
  const activeRooms = useMemo(() => {
    const map = new Map<string, string>();
    for (const avatar of avatars.values()) {
      if (avatar.state !== 'returning') {
        const config = AGENTS[avatar.agentName];
        if (config) {
          map.set(avatar.targetRoomId, config.accentColor);
        }
      }
    }
    return map;
  }, [avatars]);

  // Count active (non-terminal) agents per patient encounter for bed indicators
  const patientActiveCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const avatar of avatars.values()) {
      if (avatar.state !== 'completed' && avatar.state !== 'failed' && avatar.state !== 'returning') {
        counts.set(avatar.encounterId, (counts.get(avatar.encounterId) ?? 0) + 1);
      }
    }
    return counts;
  }, [avatars]);

  return (
    <>
      <Lighting />
      <CameraRig preset={cameraPreset} followTarget={null} />

      {/* Full hospital floor plan: rooms, hallway, connector strips */}
      <HospitalFloor activeRooms={activeRooms} />

      {/* One patient bed per exam room */}
      {PATIENTS.map((patient) => {
        const count = patientActiveCounts.get(patient.encounterId) ?? 0;
        return (
          <PatientBed
            key={patient.encounterId}
            patient={patient}
            isActive={count > 0}
            activeAgentCount={count}
          />
        );
      })}

      {/* One avatar per active agent-encounter pair */}
      {Array.from(avatars.values()).map((avatar) => {
        const agentConfig: AgentConfig | undefined = AGENTS[avatar.agentName];
        if (!agentConfig) return null;
        return (
          <AgentAvatar
            key={avatar.id}
            config={agentConfig}
            avatarState={avatar.state}
            waypoints={avatar.waypoints}
            targetRoomId={avatar.targetRoomId}
            confidence={avatar.confidence}
          />
        );
      })}

      {/* Subtle exponential fog for depth */}
      <fog attach="fog" args={['#060610', 28, 52]} />
    </>
  );
}
