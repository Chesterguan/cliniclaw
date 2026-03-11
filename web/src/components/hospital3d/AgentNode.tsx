'use client';

/**
 * AgentNode — static icosahedron node for the network graph view.
 *
 * Placed at a fixed position in the outer ring (radius 9) around the centre
 * of the NetworkScene. Visualises agent state through emissive pulsing, orbit
 * rings, glow spheres, and a point light — all driven imperatively in useFrame
 * to avoid React re-renders on every animation tick.
 *
 * Unlike AgentAvatar there is no locomotion — the node never moves.
 */

import { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import { Float, Html } from '@react-three/drei';
import * as THREE from 'three';

// ── Props ────────────────────────────────────────────────────────────────────

export type AgentNodeState = 'inactive' | 'active' | 'thinking' | 'completed' | 'failed';

interface AgentNodeProps {
  agentName: string;
  label: string;
  abbr: string;
  color: string;
  accentColor: string;
  position: [number, number, number];
  state: AgentNodeState;
}

// ── Constants ────────────────────────────────────────────────────────────────

// Icosahedron core — detail 1 gives a nice faceted polyhedron feel
const CORE_SCALE = 0.35;

// Inner glow: tight sphere just around the core (BackSide — renders from inside out)
const INNER_GLOW_SCALE = 0.65;

// Outer glow: larger envelope (BackSide)
const OUTER_GLOW_SCALE = 0.8;

// Orbit ring torus dimensions
const RING_RADIUS = 0.55;     // ring centre radius
const RING_TUBE   = 0.012;    // tube cross-section radius
const RING_SEG    = 6;        // tube segments (low-poly for perf)
const RING_STEPS  = 48;       // ring resolution

// Ground ring (flat, at Y = 0.01 relative to node)
const GROUND_RING_INNER = 0.58;
const GROUND_RING_OUTER = 0.72;

// Pillar beam below node
const PILLAR_BOT_R = 0.02;
const PILLAR_TOP_R = 0.06;
const PILLAR_H     = 0.5;

// Animation frequencies
const THINKING_HZ = 4.5;     // ~4-5 Hz pulse
const FLASH_DURATION = 1.0;  // seconds for completed/failed flash

// Label vertical offsets (relative to node group origin)
const ABBR_Y  =  0.75;   // abbreviation above core
const LABEL_Y = -0.72;   // full label below

// ── Component ────────────────────────────────────────────────────────────────

export function AgentNode({
  label,
  abbr,
  color,
  accentColor,
  position,
  state,
}: AgentNodeProps) {
  // ── Mesh refs ─────────────────────────────────────────────────────────────
  const coreRef        = useRef<THREE.Mesh>(null);
  const innerGlowRef   = useRef<THREE.Mesh>(null);
  const outerGlowRef   = useRef<THREE.Mesh>(null);
  const ring1Ref       = useRef<THREE.Mesh>(null);   // orbit ring A (tilted ±30°)
  const ring2Ref       = useRef<THREE.Mesh>(null);   // orbit ring B (perpendicular)
  const groundRingRef  = useRef<THREE.Mesh>(null);
  const pointLightRef  = useRef<THREE.PointLight>(null);

  // ── Per-frame allocation avoidance ───────────────────────────────────────
  const _col = useMemo(() => new THREE.Color(color), [color]);

  // Timestamp of most-recent entry into a terminal state (completed/failed)
  const terminalStartRef = useRef<number | null>(null);
  const prevStateRef     = useRef<AgentNodeState>(state);

  // ── Animation ─────────────────────────────────────────────────────────────

  useFrame((_state, delta) => {
    const dt = Math.min(delta, 0.1);
    const t  = performance.now() / 1000;

    const isActive    = state === 'active';
    const isThinking  = state === 'thinking';
    const isCompleted = state === 'completed';
    const isFailed    = state === 'failed';

    // Record entry into terminal state
    if (
      (isCompleted || isFailed) &&
      prevStateRef.current !== 'completed' &&
      prevStateRef.current !== 'failed'
    ) {
      terminalStartRef.current = t;
    }
    prevStateRef.current = state;

    const flashElapsed = terminalStartRef.current != null
      ? t - terminalStartRef.current
      : FLASH_DURATION + 1;

    // ── Core icosahedron ───────────────────────────────────────────────────

    if (coreRef.current) {
      const mat = coreRef.current.material as THREE.MeshStandardMaterial;

      if (isFailed) {
        _col.set('#ef4444');
        mat.color.copy(_col);
        mat.emissive.copy(_col);
        mat.emissiveIntensity = flashElapsed < FLASH_DURATION
          ? 1.4 * (1 - flashElapsed / FLASH_DURATION) + 0.3
          : 0.3;
        mat.opacity = 1.0;
      } else if (isCompleted) {
        _col.set(flashElapsed < FLASH_DURATION ? '#22c55e' : color);
        mat.color.copy(_col);
        mat.emissive.copy(_col);
        mat.emissiveIntensity = flashElapsed < FLASH_DURATION
          ? 1.4 * (1 - flashElapsed / FLASH_DURATION)
          : 0.2;
        mat.opacity = 1.0;
      } else if (isThinking) {
        // Strong pulse 0.8–1.1 at THINKING_HZ
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2) + 1) / 2; // 0..1
        _col.set(color);
        mat.color.copy(_col);
        mat.emissive.set(accentColor);
        mat.emissiveIntensity = 0.8 + pulse * 0.3;
        mat.opacity = 1.0;
        // Gentle scale breathe
        coreRef.current.scale.setScalar(CORE_SCALE * (1 + pulse * 0.06));
      } else if (isActive) {
        _col.set(color);
        mat.color.copy(_col);
        mat.emissive.copy(_col);
        mat.emissiveIntensity = 0.4;
        mat.opacity = 1.0;
        coreRef.current.scale.setScalar(CORE_SCALE);
      } else {
        // inactive
        _col.set(color);
        mat.color.copy(_col);
        mat.emissive.copy(_col);
        mat.emissiveIntensity = 0.12;
        mat.opacity = 0.7;
        coreRef.current.scale.setScalar(CORE_SCALE);
      }

      // Slow idle rotation on the core
      coreRef.current.rotation.y += dt * (isThinking ? 0.8 : isActive ? 0.35 : 0.12);
      coreRef.current.rotation.x += dt * (isThinking ? 0.5 : 0.08);
    }

    // ── Inner glow (BackSide sphere) ───────────────────────────────────────

    if (innerGlowRef.current) {
      const mat = innerGlowRef.current.material as THREE.MeshBasicMaterial;
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2) + 1) / 2;
        mat.color.set(accentColor);
        mat.opacity = 0.12 + pulse * 0.18;
        innerGlowRef.current.visible = true;
        innerGlowRef.current.scale.setScalar(INNER_GLOW_SCALE * (1 + pulse * 0.1));
      } else if (isActive) {
        mat.color.set(color);
        mat.opacity = 0.07;
        innerGlowRef.current.visible = true;
        innerGlowRef.current.scale.setScalar(INNER_GLOW_SCALE);
      } else {
        innerGlowRef.current.visible = false;
      }
    }

    // ── Outer glow (BackSide sphere) ───────────────────────────────────────

    if (outerGlowRef.current) {
      const mat = outerGlowRef.current.material as THREE.MeshBasicMaterial;
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2 + 0.5) + 1) / 2;
        mat.color.set(accentColor);
        mat.opacity = 0.06 + pulse * 0.10;
        outerGlowRef.current.visible = true;
        outerGlowRef.current.scale.setScalar(OUTER_GLOW_SCALE * (1 + pulse * 0.12));
      } else if (isActive) {
        mat.color.set(color);
        mat.opacity = 0.04;
        outerGlowRef.current.visible = true;
        outerGlowRef.current.scale.setScalar(OUTER_GLOW_SCALE);
      } else {
        outerGlowRef.current.visible = false;
      }
    }

    // ── Orbit rings ────────────────────────────────────────────────────────

    const ringSpeed = isThinking ? Math.PI * 4.5 : isActive ? Math.PI * 1.8 : Math.PI * 0.4;

    if (ring1Ref.current) {
      ring1Ref.current.rotation.z += dt * ringSpeed;
      const mat = ring1Ref.current.material as THREE.MeshBasicMaterial;
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2) + 1) / 2;
        mat.opacity = 0.35 + pulse * 0.45;
        mat.color.set(accentColor);
        ring1Ref.current.visible = true;
      } else if (isActive) {
        mat.opacity = 0.25;
        mat.color.set(accentColor);
        ring1Ref.current.visible = true;
      } else {
        mat.opacity = 0.1;
        mat.color.set(color);
        ring1Ref.current.visible = true;
      }
    }

    if (ring2Ref.current) {
      ring2Ref.current.rotation.x += dt * ringSpeed * 0.7;
      const mat = ring2Ref.current.material as THREE.MeshBasicMaterial;
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2 + Math.PI) + 1) / 2;
        mat.opacity = 0.25 + pulse * 0.35;
        mat.color.set(accentColor);
        ring2Ref.current.visible = true;
      } else if (isActive) {
        mat.opacity = 0.18;
        mat.color.set(color);
        ring2Ref.current.visible = true;
      } else {
        mat.opacity = 0.08;
        mat.color.set(color);
        ring2Ref.current.visible = true;
      }
    }

    // ── Ground ring ────────────────────────────────────────────────────────

    if (groundRingRef.current) {
      const mat = groundRingRef.current.material as THREE.MeshBasicMaterial;
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2) + 1) / 2;
        // Expanding pulse: scale oscillates outward
        groundRingRef.current.scale.setScalar(1.0 + pulse * 0.25);
        mat.opacity = 0.25 + pulse * 0.25;
        mat.color.set(accentColor);
        groundRingRef.current.visible = true;
      } else if (isActive) {
        groundRingRef.current.scale.setScalar(1.0);
        mat.opacity = 0.12;
        mat.color.set(accentColor);
        groundRingRef.current.visible = true;
      } else {
        groundRingRef.current.scale.setScalar(1.0);
        mat.opacity = 0.05;
        mat.color.set(color);
        groundRingRef.current.visible = true;
      }
    }

    // ── Point light ────────────────────────────────────────────────────────

    if (pointLightRef.current) {
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_HZ * Math.PI * 2) + 1) / 2;
        pointLightRef.current.intensity = 1.5 + pulse * 1.2;
        pointLightRef.current.distance = 5;
        pointLightRef.current.color.set(accentColor);
        pointLightRef.current.visible = true;
      } else if (isActive) {
        pointLightRef.current.intensity = 0.8;
        pointLightRef.current.distance = 4;
        pointLightRef.current.color.set(color);
        pointLightRef.current.visible = true;
      } else if (isCompleted && flashElapsed < FLASH_DURATION) {
        pointLightRef.current.intensity = 3.0 * (1 - flashElapsed / FLASH_DURATION);
        pointLightRef.current.distance = 5;
        pointLightRef.current.color.set('#22c55e');
        pointLightRef.current.visible = true;
      } else if (isFailed && flashElapsed < FLASH_DURATION) {
        pointLightRef.current.intensity = 2.5 * (1 - flashElapsed / FLASH_DURATION);
        pointLightRef.current.distance = 4;
        pointLightRef.current.color.set('#ef4444');
        pointLightRef.current.visible = true;
      } else {
        pointLightRef.current.visible = false;
      }
    }
  });

  // ── JSX ───────────────────────────────────────────────────────────────────

  return (
    <Float
      position={position}
      speed={state === 'thinking' ? 3 : 1.2}
      rotationIntensity={0}
      floatIntensity={state === 'thinking' ? 0.4 : 0.15}
    >
      {/* ── Core icosahedron ─────────────────────────────────────────── */}
      <mesh ref={coreRef} scale={CORE_SCALE}>
        <icosahedronGeometry args={[1, 1]} />
        <meshStandardMaterial
          color={color}
          emissive={color}
          emissiveIntensity={0.12}
          metalness={0.6}
          roughness={0.3}
          transparent
          opacity={1.0}
        />
      </mesh>

      {/* ── Inner glow sphere (BackSide — renders from inside) ───────── */}
      <mesh ref={innerGlowRef} scale={INNER_GLOW_SCALE} visible={false}>
        <sphereGeometry args={[1, 20, 16]} />
        <meshBasicMaterial
          color={accentColor}
          transparent
          opacity={0.08}
          depthWrite={false}
          side={THREE.BackSide}
        />
      </mesh>

      {/* ── Outer glow sphere (BackSide) ─────────────────────────────── */}
      <mesh ref={outerGlowRef} scale={OUTER_GLOW_SCALE} visible={false}>
        <sphereGeometry args={[1, 20, 16]} />
        <meshBasicMaterial
          color={accentColor}
          transparent
          opacity={0.05}
          depthWrite={false}
          side={THREE.BackSide}
        />
      </mesh>

      {/* ── Orbit ring A — tilted 30° on X axis ──────────────────────── */}
      <mesh ref={ring1Ref} rotation={[Math.PI / 6, 0, 0]}>
        <torusGeometry args={[RING_RADIUS, RING_TUBE, RING_SEG, RING_STEPS]} />
        <meshBasicMaterial
          color={accentColor}
          transparent
          opacity={0.25}
          depthWrite={false}
        />
      </mesh>

      {/* ── Orbit ring B — perpendicular to ring A ───────────────────── */}
      <mesh ref={ring2Ref} rotation={[0, 0, Math.PI / 2]}>
        <torusGeometry args={[RING_RADIUS, RING_TUBE, RING_SEG, RING_STEPS]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.18}
          depthWrite={false}
        />
      </mesh>

      {/* ── Pillar beam below — frustum cylinder anchoring node to ground ─
           CylinderGeometry(radiusTop, radiusBottom, height, radialSegments)
           Narrow at the top (near the core), wider at the bottom (ground ring). */}
      <mesh position={[0, -(PILLAR_H / 2 + CORE_SCALE), 0]}>
        <cylinderGeometry args={[PILLAR_BOT_R, PILLAR_TOP_R, PILLAR_H, 6]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.3}
          depthWrite={false}
        />
      </mesh>

      {/* ── Ground ring — flat ring at foot of pillar ─────────────────── */}
      <mesh
        ref={groundRingRef}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, -(CORE_SCALE + PILLAR_H) + 0.01, 0]}
      >
        <ringGeometry args={[GROUND_RING_INNER, GROUND_RING_OUTER, 48]} />
        <meshBasicMaterial
          color={accentColor}
          transparent
          opacity={0.08}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* ── Point light — driven imperatively, starts invisible ────────── */}
      <pointLight
        ref={pointLightRef}
        color={accentColor}
        intensity={0}
        distance={4}
        decay={2}
        visible={false}
      />

      {/* ── HTML label: abbreviation above ───────────────────────────── */}
      <Html
        position={[0, ABBR_Y, 0]}
        center
        distanceFactor={18}
        style={{ pointerEvents: 'none' }}
      >
        <span style={{
          background: `${color}dd`,
          color: '#ffffff',
          fontSize: 10,
          fontWeight: 800,
          padding: '2px 7px',
          borderRadius: 4,
          letterSpacing: '0.07em',
          boxShadow: `0 0 8px ${color}55`,
          whiteSpace: 'nowrap',
          display: 'block',
          textAlign: 'center',
        }}>
          {abbr}
        </span>
      </Html>

      {/* ── HTML label: full name below ───────────────────────────────── */}
      <Html
        position={[0, LABEL_Y, 0]}
        center
        distanceFactor={18}
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
          {label}
        </span>
      </Html>
    </Float>
  );
}
