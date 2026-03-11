'use client';

/**
 * PatientBed — physical patient indicator placed inside an exam room.
 *
 * Renders a stylized bed (flat box + headboard) with a patient sphere above it.
 * The sphere pulses with a heartbeat animation and emits expanding pulse rings
 * when an agent is actively working on the patient.
 *
 * Position is derived from the patient's roomId via ROOM_MAP — the bed sits at
 * the back-center of the exam room, leaving the open front accessible to agents.
 */

import { useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import { Text } from '@react-three/drei';
import * as THREE from 'three';
import { ROOM_MAP, type PatientConfig } from '@/lib/hospital3d/constants';

interface PatientBedProps {
  patient: PatientConfig;
  isActive?: boolean;
  /** Number of agents currently working on this patient */
  activeAgentCount?: number;
}

// Bed geometry dimensions
const BED_W = 1.2;
const BED_H = 0.4;
const BED_D = 0.6;
const BED_ELEVATION = 0.2;   // Y center of mattress box
const HEADBOARD_W = BED_W;
const HEADBOARD_H = 0.4;     // extra height above mattress
const HEADBOARD_D = 0.06;
const SPHERE_RADIUS = 0.15;
const SPHERE_Y = BED_ELEVATION + BED_H / 2 + 0.45; // float above mattress top

export function PatientBed({
  patient,
  isActive = false,
  activeAgentCount = 0,
}: PatientBedProps) {
  const sphereRef = useRef<THREE.Mesh>(null);
  const sphereGlowRef = useRef<THREE.Mesh>(null);
  const pulse1Ref = useRef<THREE.Mesh>(null);
  const pulse2Ref = useRef<THREE.Mesh>(null);
  const mattressRef = useRef<THREE.Mesh>(null);

  // Look up the room this patient is assigned to
  const room = ROOM_MAP[patient.roomId];
  if (!room) return null;

  const [rx, , rz] = room.position;
  const halfD = room.size[1] / 2;

  // Place bed toward the back wall of the room (opposite the open front)
  const isNorth = rz < 0;
  // North rooms open south — back is north (more negative Z), so bed sits at back
  const bedZ = isNorth ? rz - halfD + BED_D / 2 + 0.25 : rz + halfD - BED_D / 2 - 0.25;
  const bedX = rx;

  const headboardTopY = BED_ELEVATION + BED_H / 2 + HEADBOARD_H / 2;

  useFrame(() => {
    const t = performance.now() / 1000;

    // Sphere heartbeat — cubic ease for sharp beat feel
    if (sphereRef.current) {
      const beat = Math.pow(Math.max(0, Math.sin(t * 2.5)), 3) * 0.04;
      const base = SPHERE_RADIUS + (isActive ? activeAgentCount * 0.01 : 0);
      const scale = (base + beat) / SPHERE_RADIUS; // normalize to geometry radius
      sphereRef.current.scale.setScalar(scale);

      const mat = sphereRef.current.material as THREE.MeshStandardMaterial;
      mat.emissiveIntensity = isActive
        ? 0.4 + beat * 6 + Math.min(activeAgentCount - 1, 2) * 0.1
        : 0.1 + beat * 3;
    }

    // Sphere glow halo
    if (sphereGlowRef.current) {
      const mat = sphereGlowRef.current.material as THREE.MeshBasicMaterial;
      if (isActive) {
        const pulse = Math.sin(t * 1.8) * 0.5 + 0.5;
        sphereGlowRef.current.scale.setScalar(2.4 + pulse * 0.3);
        mat.opacity = 0.05 + pulse * 0.04;
      } else {
        sphereGlowRef.current.scale.setScalar(1.8);
        mat.opacity = 0.02;
      }
    }

    // Expanding pulse rings when active
    [pulse1Ref, pulse2Ref].forEach((ref, i) => {
      if (!ref.current) return;
      const mat = ref.current.material as THREE.MeshBasicMaterial;
      if (isActive) {
        const offset = i * 0.65;
        const pulseT = ((t * 0.75 + offset) % 1.5) / 1.5;
        ref.current.scale.setScalar(0.3 + pulseT * 1.6);
        mat.opacity = 0.22 * (1 - pulseT);
      } else {
        mat.opacity = 0;
      }
    });

    // Mattress subtle active tint
    if (mattressRef.current) {
      const mat = mattressRef.current.material as THREE.MeshStandardMaterial;
      mat.emissiveIntensity = isActive ? 0.03 + Math.sin(t * 2) * 0.01 : 0.0;
    }
  });

  return (
    <group position={[bedX, 0, bedZ]}>
      {/* ── Bed mattress ─────────────────────────────────────────────── */}
      <mesh
        ref={mattressRef}
        position={[0, BED_ELEVATION, 0]}
        castShadow
        receiveShadow
      >
        <boxGeometry args={[BED_W, BED_H, BED_D]} />
        <meshStandardMaterial
          color="#1a1f2e"
          emissive={patient.color}
          emissiveIntensity={0.0}
          roughness={0.8}
          metalness={0.05}
        />
      </mesh>

      {/* ── Headboard — slightly taller thin box at the back of the bed ── */}
      <mesh
        position={[
          0,
          BED_ELEVATION + BED_H / 2 + HEADBOARD_H / 2,
          // headboard at the back edge of the mattress
          isNorth ? -(BED_D / 2 - HEADBOARD_D / 2) : BED_D / 2 - HEADBOARD_D / 2,
        ]}
        castShadow
      >
        <boxGeometry args={[HEADBOARD_W, HEADBOARD_H, HEADBOARD_D]} />
        <meshStandardMaterial
          color="#131720"
          roughness={0.75}
          metalness={0.1}
        />
      </mesh>

      {/* ── Bed frame legs (decorative thin strips) ──────────────────── */}
      {([-1, 1] as const).map((sx) =>
        ([-1, 1] as const).map((sz) => (
          <mesh
            key={`leg-${sx}-${sz}`}
            position={[
              sx * (BED_W / 2 - 0.06),
              BED_ELEVATION / 2,
              sz * (BED_D / 2 - 0.06),
            ]}
          >
            <boxGeometry args={[0.05, BED_ELEVATION, 0.05]} />
            <meshStandardMaterial color="#111520" roughness={0.6} metalness={0.3} />
          </mesh>
        ))
      )}

      {/* ── Patient indicator sphere ──────────────────────────────────── */}
      {/* Outer glow halo */}
      <mesh ref={sphereGlowRef} position={[0, SPHERE_Y, 0]} scale={1.8}>
        <sphereGeometry args={[SPHERE_RADIUS, 24, 24]} />
        <meshBasicMaterial
          color={patient.color}
          transparent
          opacity={0.02}
          depthWrite={false}
          side={THREE.BackSide}
        />
      </mesh>

      {/* Core sphere */}
      <mesh ref={sphereRef} position={[0, SPHERE_Y, 0]} castShadow>
        <sphereGeometry args={[SPHERE_RADIUS, 24, 24]} />
        <meshStandardMaterial
          color={patient.color}
          emissive={patient.color}
          emissiveIntensity={0.1}
          roughness={0.35}
          metalness={0.15}
          transparent
          opacity={0.92}
        />
      </mesh>

      {/* Pulse ring 1 */}
      <mesh
        ref={pulse1Ref}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, BED_ELEVATION + BED_H / 2 + 0.01, 0]}
        scale={0.3}
      >
        <ringGeometry args={[0.85, 1.0, 48]} />
        <meshBasicMaterial
          color={patient.color}
          transparent
          opacity={0}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Pulse ring 2 (staggered) */}
      <mesh
        ref={pulse2Ref}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, BED_ELEVATION + BED_H / 2 + 0.01, 0]}
        scale={0.3}
      >
        <ringGeometry args={[0.85, 1.0, 48]} />
        <meshBasicMaterial
          color={patient.color}
          transparent
          opacity={0}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Point light when agent is actively working on patient */}
      {isActive && (
        <pointLight
          position={[0, SPHERE_Y, 0]}
          color={patient.color}
          intensity={0.5 + Math.min(activeAgentCount - 1, 2) * 0.2}
          distance={3.5}
          decay={2}
        />
      )}

      {/* ── Patient label ─────────────────────────────────────────────── */}
      {/* Name above sphere */}
      <Text
        position={[0, SPHERE_Y + SPHERE_RADIUS + 0.25, 0]}
        fontSize={0.25}
        color={patient.color}
        anchorX="center"
        anchorY="bottom"
        renderOrder={1}
        depthOffset={1}
      >
        {patient.name}
      </Text>

      {/* Condition below name */}
      <Text
        position={[0, SPHERE_Y + SPHERE_RADIUS + 0.02, 0]}
        fontSize={0.2}
        color="#64748b"
        anchorX="center"
        anchorY="top"
        renderOrder={1}
        depthOffset={1}
      >
        {patient.condition}
      </Text>

      {/* Encounter ID — very small, below bed */}
      <Text
        position={[0, 0.02, BED_D / 2 + 0.05]}
        fontSize={0.15}
        color="#334155"
        anchorX="center"
        anchorY="bottom"
        renderOrder={1}
        depthOffset={1}
      >
        {patient.encounterId}
      </Text>
    </group>
  );
}
