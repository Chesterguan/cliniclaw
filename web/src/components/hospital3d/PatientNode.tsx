'use client';

/**
 * PatientNode — static patient sphere for the network graph view.
 *
 * Placed at a fixed position in the inner ring (radius 3.8) around the centre
 * of the NetworkScene. Visualises patient activity through heartbeat animation,
 * expanding pulse rings, and an optional orbiting clinician-review indicator.
 *
 * All animation is driven imperatively in useFrame — no React state updates
 * per frame. The Float wrapper adds a subtle idle drift.
 */

import { useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import { Float, Html } from '@react-three/drei';
import * as THREE from 'three';

// ── Props ────────────────────────────────────────────────────────────────────

interface PatientNodeProps {
  name: string;
  condition: string;
  color: string;
  position: [number, number, number];
  isActive: boolean;
  /** Number of agents currently connected to this patient. */
  activeCount: number;
  /** Show orbiting gold clinician-review indicator. */
  clinicianReview?: boolean;
}

// ── Constants ────────────────────────────────────────────────────────────────

const CORE_RADIUS  = 0.3;
const GLOW_RADIUS  = 0.5;

// Heartbeat cross — small octahedron that rotates above the sphere
const CROSS_Y = CORE_RADIUS + 0.25;

// Pulse rings — flat, staggered expansions at the equator of the sphere
const PULSE_Y = 0.01;  // just above floor level (relative to group origin)

// Gold clinician-review diamond orbit
const CLINICIAN_ORBIT_R = 0.55;
const CLINICIAN_Y       = 0.1;
const CLINICIAN_COLOR   = '#fbbf24';

// Label vertical offsets (relative to group origin)
const NAME_Y      =  CORE_RADIUS + 0.45;
const CONDITION_Y =  CORE_RADIUS + 0.22;

// ── Component ────────────────────────────────────────────────────────────────

export function PatientNode({
  name,
  condition,
  color,
  position,
  isActive,
  activeCount,
  clinicianReview = false,
}: PatientNodeProps) {
  // ── Mesh refs ─────────────────────────────────────────────────────────────
  const coreRef          = useRef<THREE.Mesh>(null);
  const glowRef          = useRef<THREE.Mesh>(null);
  const crossRef         = useRef<THREE.Mesh>(null);    // octahedron heartbeat
  const pulse1Ref        = useRef<THREE.Mesh>(null);
  const pulse2Ref        = useRef<THREE.Mesh>(null);
  const pointLightRef    = useRef<THREE.PointLight>(null);
  const clinicianRef     = useRef<THREE.Mesh>(null);

  // ── Animation ─────────────────────────────────────────────────────────────

  useFrame((_state, delta) => {
    const dt = Math.min(delta, 0.1);
    const t  = performance.now() / 1000;

    // ── Core sphere ───────────────────────────────────────────────────────

    if (coreRef.current) {
      const mat = coreRef.current.material as THREE.MeshStandardMaterial;

      // Heartbeat: sharp cubic beat at ~2.5 Hz
      const beat = Math.pow(Math.max(0, Math.sin(t * 2.5)), 3) * 0.04;
      const baseR = CORE_RADIUS + (isActive ? activeCount * 0.012 : 0);
      coreRef.current.scale.setScalar((baseR + beat) / CORE_RADIUS);

      mat.emissiveIntensity = isActive
        ? 0.35 + beat * 5 + Math.min(activeCount - 1, 2) * 0.1
        : 0.1 + beat * 2.5;

      // Slight opacity: active is fully opaque, inactive slightly translucent
      mat.opacity = isActive ? 0.95 : 0.75;
    }

    // ── Glow sphere (BackSide) ─────────────────────────────────────────────

    if (glowRef.current) {
      const mat = glowRef.current.material as THREE.MeshBasicMaterial;
      if (isActive) {
        const pulse = Math.sin(t * 1.8) * 0.5 + 0.5;
        glowRef.current.scale.setScalar((GLOW_RADIUS / CORE_RADIUS) * (1 + pulse * 0.15));
        mat.opacity = 0.05 + pulse * 0.05;
      } else {
        glowRef.current.scale.setScalar(GLOW_RADIUS / CORE_RADIUS);
        mat.opacity = 0.02;
      }
    }

    // ── Heartbeat cross (octahedron) ───────────────────────────────────────

    if (crossRef.current) {
      // Slow spin normally, faster beat during active
      crossRef.current.rotation.y += dt * (isActive ? 1.8 : 0.6);
      crossRef.current.rotation.z += dt * (isActive ? 0.9 : 0.3);

      const mat = crossRef.current.material as THREE.MeshStandardMaterial;
      const beat = Math.pow(Math.max(0, Math.sin(t * 2.5)), 3);
      mat.emissiveIntensity = isActive ? 0.6 + beat * 0.6 : 0.2 + beat * 0.2;
    }

    // ── Pulse rings ────────────────────────────────────────────────────────

    ([pulse1Ref, pulse2Ref] as const).forEach((ref, i) => {
      if (!ref.current) return;
      const mat = ref.current.material as THREE.MeshBasicMaterial;
      if (isActive) {
        const offset = i * 0.65;
        const pulseT = ((t * 0.75 + offset) % 1.5) / 1.5;
        ref.current.scale.setScalar(0.25 + pulseT * 1.8);
        mat.opacity = 0.28 * (1 - pulseT);
      } else {
        mat.opacity = 0;
        ref.current.scale.setScalar(0.25);
      }
    });

    // ── Point light ────────────────────────────────────────────────────────

    if (pointLightRef.current) {
      if (isActive) {
        const beat = Math.pow(Math.max(0, Math.sin(t * 2.5)), 3);
        pointLightRef.current.intensity = 0.5 + Math.min(activeCount - 1, 2) * 0.2 + beat * 0.3;
        pointLightRef.current.visible = true;
      } else {
        pointLightRef.current.visible = false;
      }
    }

    // ── Clinician indicator — orbiting gold diamond ────────────────────────

    if (clinicianRef.current) {
      if (clinicianReview) {
        const angle = t * 1.4;
        clinicianRef.current.position.x = Math.cos(angle) * CLINICIAN_ORBIT_R;
        clinicianRef.current.position.z = Math.sin(angle) * CLINICIAN_ORBIT_R;
        clinicianRef.current.position.y = CLINICIAN_Y + Math.sin(t * 2.2) * 0.08;
        clinicianRef.current.rotation.y += dt * 2.5;
        clinicianRef.current.visible = true;

        const mat = clinicianRef.current.material as THREE.MeshStandardMaterial;
        mat.emissiveIntensity = 0.5 + Math.sin(t * 3) * 0.3;
      } else {
        clinicianRef.current.visible = false;
      }
    }
  });

  // ── JSX ───────────────────────────────────────────────────────────────────

  return (
    <Float
      position={position}
      speed={isActive ? 2.5 : 1.0}
      rotationIntensity={0}
      floatIntensity={isActive ? 0.3 : 0.12}
    >
      {/* ── Core sphere ─────────────────────────────────────────────── */}
      <mesh ref={coreRef} scale={CORE_RADIUS} castShadow>
        <sphereGeometry args={[1, 24, 24]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.1}
          roughness={0.35}
          metalness={0.15}
          transparent
          opacity={0.85}
        />
      </mesh>

      {/* ── Glow sphere (BackSide — renders inside-out) ──────────────── */}
      <mesh ref={glowRef} scale={GLOW_RADIUS}>
        <sphereGeometry args={[1, 20, 20]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.025}
          depthWrite={false}
          side={THREE.BackSide}
        />
      </mesh>

      {/* ── Heartbeat cross: small white octahedron above the sphere ──── */}
      <mesh ref={crossRef} position={[0, CROSS_Y, 0]} scale={0.07}>
        <octahedronGeometry args={[1, 0]} />
        <meshStandardMaterial
          color="#ffffff"
          emissive="#ffffff"
          emissiveIntensity={0.2}
          roughness={0.4}
          metalness={0.2}
        />
      </mesh>

      {/* ── Pulse ring 1 ─────────────────────────────────────────────── */}
      <mesh
        ref={pulse1Ref}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, PULSE_Y, 0]}
        scale={0.25}
      >
        <ringGeometry args={[0.85, 1.0, 48]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* ── Pulse ring 2 (staggered phase) ───────────────────────────── */}
      <mesh
        ref={pulse2Ref}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, PULSE_Y, 0]}
        scale={0.25}
      >
        <ringGeometry args={[0.85, 1.0, 48]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* ── Point light (active only) ─────────────────────────────────── */}
      <pointLight
        ref={pointLightRef}
        color={color}
        intensity={0.5}
        distance={3.5}
        decay={2}
        visible={false}
      />

      {/* ── Clinician review: orbiting gold diamond ───────────────────── */}
      {/* Positioned imperatively in useFrame, initial position off to the side */}
      <mesh
        ref={clinicianRef}
        position={[CLINICIAN_ORBIT_R, CLINICIAN_Y, 0]}
        scale={0.09}
        visible={clinicianReview}
      >
        <octahedronGeometry args={[1, 0]} />
        <meshStandardMaterial
          color={CLINICIAN_COLOR}
          emissive={CLINICIAN_COLOR}
          emissiveIntensity={0.5}
          roughness={0.3}
          metalness={0.4}
        />
      </mesh>

      {/* ── HTML labels ──────────────────────────────────────────────── */}
      <Html
        position={[0, NAME_Y, 0]}
        center
        distanceFactor={14}
        style={{ pointerEvents: 'none' }}
      >
        <span style={{
          color: color,
          fontSize: 10,
          fontWeight: 700,
          textShadow: '0 1px 4px rgba(0,0,0,0.8)',
          whiteSpace: 'nowrap',
          display: 'block',
          textAlign: 'center',
        }}>
          {name}
        </span>
      </Html>

      <Html
        position={[0, CONDITION_Y, 0]}
        center
        distanceFactor={14}
        style={{ pointerEvents: 'none' }}
      >
        <span style={{
          color: '#64748b',
          fontSize: 9,
          fontWeight: 500,
          textShadow: '0 1px 3px rgba(0,0,0,0.7)',
          whiteSpace: 'nowrap',
          display: 'block',
          textAlign: 'center',
        }}>
          {condition}
        </span>
      </Html>
    </Float>
  );
}
