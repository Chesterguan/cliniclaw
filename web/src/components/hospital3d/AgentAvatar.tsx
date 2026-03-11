'use client';

// AgentAvatar — moving capsule humanoid that walks between hospital rooms.
// Replaces the static abstract AgentNode with a physically-grounded avatar
// that follows waypoint paths, faces direction of travel, and shows
// state-driven visual effects (glow, pulse, flash) via useFrame only.

import { useRef, useEffect } from 'react';
import { useFrame } from '@react-three/fiber';
import { Html } from '@react-three/drei';
import * as THREE from 'three';

import type { AgentConfig, AvatarState } from '@/lib/hospital3d/constants';
import { getAgentIdlePosition } from '@/lib/hospital3d/layout';
import { WALK_SPEED } from '@/lib/hospital3d/layout';

// ─── Props ──────────────────────────────────────────────────────────────────

interface AgentAvatarProps {
  config: AgentConfig;
  avatarState: AvatarState;
  /** Waypoint path to follow when avatarState is 'walking' or 'returning'. */
  waypoints: [number, number, number][];
  /** Current target room — used for bedside logic upstream; not used internally. */
  targetRoomId: string | null;
  /** Confidence score from agent_completed event; drives completed flash intensity. */
  confidence: number | null;
}

// ─── Constants ──────────────────────────────────────────────────────────────

// Capsule geometry: radius=0.18, height=0.7 (the straight cylinder part).
// Total half-height = height/2 + radius = 0.35 + 0.18 = 0.53.
// We want feet at Y=0, so the group origin is at foot level and
// the body mesh sits at Y offset = 0.53 (center of capsule at mid-body).
const BODY_RADIUS = 0.18;
const BODY_HEIGHT = 0.7;
const BODY_CENTER_Y = BODY_HEIGHT / 2 + BODY_RADIUS; // 0.53 — body center above floor

// Head sits on top of the capsule: capsule top = 0.53 + 0.53 = 1.06, head center above that
const HEAD_RADIUS = 0.14;
const HEAD_Y = BODY_CENTER_Y + BODY_RADIUS + HEAD_RADIUS; // 0.53 + 0.18 + 0.14 = 0.85

// Badge and label vertical positions
const BADGE_Y = HEAD_Y + HEAD_RADIUS + 0.22; // above head
const LABEL_Y = -0.12;                        // below feet

// Glow aura is a sphere around the full avatar, centered at mid-body
const AURA_Y = BODY_CENTER_Y;
const AURA_RADIUS = 0.6;

// Point light position (inside aura, at body center)
const LIGHT_Y = BODY_CENTER_Y;

// Orbit ring (horizontal, visible during thinking)
const RING_Y = BODY_CENTER_Y;
const RING_RADIUS = 0.5;

// Thinking pulse frequency (radians/sec)
const THINKING_PULSE_HZ = 4.0; // ~4Hz
const WRITING_PULSE_HZ = 2.0;

// How long the completed/failed flash lasts (seconds)
const FLASH_DURATION = 1.0;

// Minimum distance to a waypoint to consider it "reached"
const WAYPOINT_REACH_DIST = 0.08;

// ─── Component ──────────────────────────────────────────────────────────────

export function AgentAvatar({
  config,
  avatarState,
  waypoints,
  confidence,
}: AgentAvatarProps) {
  // ── Scene graph refs ─────────────────────────────────────────────────────

  // Root group — we move this to position the avatar
  const groupRef = useRef<THREE.Group>(null);

  // Body capsule
  const bodyRef = useRef<THREE.Mesh>(null);

  // Head sphere
  const headRef = useRef<THREE.Mesh>(null);

  // Ground shadow disc
  const shadowRef = useRef<THREE.Mesh>(null);

  // Glow aura sphere (visible during thinking/writing)
  const auraRef = useRef<THREE.Mesh>(null);

  // Point light inside aura
  const pointLightRef = useRef<THREE.PointLight>(null);

  // Orbit ring (horizontal, spins during thinking)
  const ringRef = useRef<THREE.Mesh>(null);

  // ── Mutable state refs (never trigger React renders) ─────────────────────

  // Current world position of the avatar (feet at Y=0)
  const positionRef = useRef<THREE.Vector3>(
    new THREE.Vector3(...getAgentIdlePosition(config.agentName))
  );

  // Current waypoint index being walked toward
  const waypointIndexRef = useRef<number>(0);

  // Smoothed Y rotation target for facing direction of travel
  const rotationYRef = useRef<number>(0);

  // Timestamp (performance.now() / 1000) when terminal state (completed/failed) started
  const terminalStartRef = useRef<number | null>(null);

  // Tracks previous waypoints array reference to detect changes
  const prevWaypointsRef = useRef<[number, number, number][]>([]);

  // Tracks previous state to detect transitions (e.g., entering terminal state)
  const prevStateRef = useRef<AvatarState>(avatarState);

  // ── Pre-allocated THREE objects to avoid per-frame allocation ─────────────

  const _targetVec = useRef(new THREE.Vector3());
  const _moveDir = useRef(new THREE.Vector3());
  const _lightColor = useRef(new THREE.Color(config.color));
  const _flashColor = useRef(new THREE.Color());

  // ── Waypoint reset on path change ────────────────────────────────────────

  // When the waypoints array changes identity (new path assigned), reset
  // the waypoint index so the avatar walks from the start of the new path.
  // We compare by reference to avoid resetting on every render.
  useEffect(() => {
    if (waypoints !== prevWaypointsRef.current) {
      waypointIndexRef.current = 0;
      prevWaypointsRef.current = waypoints;
    }
  }, [waypoints]);

  // ── Terminal state entry timestamp ───────────────────────────────────────

  useEffect(() => {
    const entering =
      (avatarState === 'completed' || avatarState === 'failed') &&
      prevStateRef.current !== 'completed' &&
      prevStateRef.current !== 'failed';

    if (entering) {
      terminalStartRef.current = performance.now() / 1000;
    }

    prevStateRef.current = avatarState;
  }, [avatarState]);

  // ── Animation loop ────────────────────────────────────────────────────────

  useFrame((_state, delta) => {
    // Cap delta to avoid huge jumps after tab switch / focus loss
    const dt = Math.min(delta, 0.1);
    const t = performance.now() / 1000;

    const isWalking = avatarState === 'walking' || avatarState === 'returning';
    const isThinking = avatarState === 'thinking';
    const isWorking = avatarState === 'working';
    const isWriting = avatarState === 'writing';
    const isCompleted = avatarState === 'completed';
    const isFailed = avatarState === 'failed';
    const isIdle = avatarState === 'idle';
    const isTerminal = isCompleted || isFailed;

    const pos = positionRef.current;

    // ── Movement ────────────────────────────────────────────────────────────

    if (isWalking && waypoints.length > 0) {
      const idx = waypointIndexRef.current;

      if (idx < waypoints.length) {
        const wp = waypoints[idx];
        _targetVec.current.set(wp[0], wp[1], wp[2]);

        const dist = pos.distanceTo(_targetVec.current);

        if (dist < WAYPOINT_REACH_DIST) {
          // Snap to waypoint and advance
          pos.copy(_targetVec.current);
          waypointIndexRef.current = idx + 1;
        } else {
          // Move toward waypoint at WALK_SPEED (frame-rate independent via delta)
          _moveDir.current
            .copy(_targetVec.current)
            .sub(pos)
            .normalize();

          const step = Math.min(WALK_SPEED * dt, dist);
          pos.addScaledVector(_moveDir.current, step);

          // Smooth rotation to face direction of travel (only on XZ plane)
          const targetAngle = Math.atan2(_moveDir.current.x, _moveDir.current.z);
          // Shortest-angle lerp: unwrap delta to [-π, π] then blend
          let angleDelta = targetAngle - rotationYRef.current;
          while (angleDelta > Math.PI) angleDelta -= Math.PI * 2;
          while (angleDelta < -Math.PI) angleDelta += Math.PI * 2;
          rotationYRef.current += angleDelta * Math.min(dt * 12, 1);
        }
      }
    }

    // Idle: ensure avatar is at its home position (teleport on first frame)
    if (isIdle) {
      const home = getAgentIdlePosition(config.agentName);
      pos.set(home[0], home[1], home[2]);
    }

    // Apply position and rotation to group
    if (groupRef.current) {
      groupRef.current.position.copy(pos);
      groupRef.current.rotation.y = rotationYRef.current;
    }

    // ── Body visual ─────────────────────────────────────────────────────────

    if (bodyRef.current) {
      const mat = bodyRef.current.material as THREE.MeshStandardMaterial;

      // Determine display color based on state
      let bodyColor: string;
      let emissiveColor: string;
      let emissiveIntensity: number;
      let opacity: number;
      let rotationZ = 0;

      if (isFailed) {
        // Red tint, slight tilt
        bodyColor = '#ef4444';
        emissiveColor = '#ef4444';
        const flashElapsed = terminalStartRef.current != null
          ? t - terminalStartRef.current : FLASH_DURATION;
        emissiveIntensity = flashElapsed < FLASH_DURATION
          ? 1.2 * (1 - flashElapsed / FLASH_DURATION)
          : 0.3;
        opacity = 1.0;
        rotationZ = 0.1;
      } else if (isCompleted) {
        // Green flash fading back to agent color
        const flashElapsed = terminalStartRef.current != null
          ? t - terminalStartRef.current : FLASH_DURATION;
        if (flashElapsed < FLASH_DURATION) {
          _flashColor.current.set('#22c55e');
          bodyColor = '#22c55e';
          emissiveColor = '#22c55e';
          emissiveIntensity = 1.4 * (1 - flashElapsed / FLASH_DURATION);
        } else {
          bodyColor = config.color;
          emissiveColor = config.color;
          emissiveIntensity = 0.2;
        }
        opacity = 1.0;
      } else if (isThinking) {
        bodyColor = config.color;
        emissiveColor = config.accentColor;
        emissiveIntensity = 0.6 + Math.sin(t * THINKING_PULSE_HZ * Math.PI * 2) * 0.3;
        opacity = 1.0;
      } else if (isWriting) {
        bodyColor = config.color;
        emissiveColor = '#22c55e';
        emissiveIntensity = 0.4 + Math.sin(t * WRITING_PULSE_HZ * Math.PI * 2) * 0.15;
        opacity = 1.0;
      } else if (isWorking) {
        bodyColor = config.color;
        emissiveColor = config.color;
        emissiveIntensity = 0.3 + Math.sin(t * 2.0 * Math.PI * 2) * 0.1;
        opacity = 1.0;
      } else if (isWalking) {
        bodyColor = config.color;
        emissiveColor = config.color;
        emissiveIntensity = 0.2;
        opacity = 1.0;
      } else {
        // idle
        bodyColor = config.color;
        emissiveColor = config.color;
        emissiveIntensity = 0.1;
        opacity = 0.7;
      }

      mat.color.set(bodyColor);
      mat.emissive.set(emissiveColor);
      mat.emissiveIntensity = emissiveIntensity;
      mat.opacity = opacity;
      bodyRef.current.rotation.z = rotationZ;
    }

    // ── Head visual ─────────────────────────────────────────────────────────

    if (headRef.current) {
      const mat = headRef.current.material as THREE.MeshStandardMaterial;

      // Head bobs slightly when walking
      let headBobY = 0;
      if (isWalking) {
        headBobY = Math.sin(t * 8) * 0.03;
      } else if (isIdle) {
        // Gentle idle bob
        headBobY = Math.sin(t * 1.5) * 0.015;
      }
      headRef.current.position.y = HEAD_Y + headBobY;

      // Head is a slightly lighter shade of agent color — computed once from config
      // We reuse emissive for head to keep it cohesive with body state
      if (isThinking) {
        mat.emissiveIntensity = 0.4 + Math.sin(t * THINKING_PULSE_HZ * Math.PI * 2) * 0.2;
      } else if (isWriting) {
        mat.emissiveIntensity = 0.3;
      } else if (isIdle) {
        mat.opacity = 0.7;
        mat.emissiveIntensity = 0.08;
      } else {
        mat.opacity = 1.0;
        mat.emissiveIntensity = 0.15;
      }

      if (isFailed) {
        mat.color.set('#ef4444');
        mat.emissive.set('#ef4444');
      } else if (isCompleted) {
        const flashElapsed = terminalStartRef.current != null
          ? t - terminalStartRef.current : FLASH_DURATION;
        if (flashElapsed < FLASH_DURATION) {
          mat.color.set('#22c55e');
          mat.emissive.set('#22c55e');
        } else {
          mat.color.set(config.accentColor);
          mat.emissive.set(config.accentColor);
        }
      } else {
        // Head is slightly lighter — accentColor gives natural lighter tone
        mat.color.set(config.accentColor);
        mat.emissive.set(config.accentColor);
      }
    }

    // ── Ground shadow ───────────────────────────────────────────────────────

    if (shadowRef.current) {
      const mat = shadowRef.current.material as THREE.MeshBasicMaterial;
      // Shadow shrinks and fades during thinking (avatar "lifts off" slightly)
      if (isThinking) {
        shadowRef.current.scale.setScalar(1.1 + Math.sin(t * THINKING_PULSE_HZ) * 0.05);
        mat.opacity = 0.25;
      } else if (isIdle) {
        shadowRef.current.scale.setScalar(0.9);
        mat.opacity = 0.12;
      } else {
        shadowRef.current.scale.setScalar(1.0);
        mat.opacity = 0.18;
      }
    }

    // ── Glow aura ───────────────────────────────────────────────────────────

    if (auraRef.current) {
      const mat = auraRef.current.material as THREE.MeshBasicMaterial;

      if (isThinking) {
        // Pulsing aura: opacity 0.05–0.15 at ~4Hz, agent accentColor
        const pulse = (Math.sin(t * THINKING_PULSE_HZ * Math.PI * 2) + 1) / 2; // 0..1
        mat.opacity = 0.05 + pulse * 0.10;
        mat.color.set(config.accentColor);
        auraRef.current.visible = true;
        auraRef.current.scale.setScalar(1.0 + pulse * 0.08);
      } else if (isWriting) {
        const pulse = (Math.sin(t * WRITING_PULSE_HZ * Math.PI * 2) + 1) / 2;
        mat.opacity = 0.04 + pulse * 0.06;
        mat.color.set('#22c55e');
        auraRef.current.visible = true;
        auraRef.current.scale.setScalar(0.85);
      } else {
        auraRef.current.visible = false;
      }
    }

    // ── Point light ─────────────────────────────────────────────────────────

    if (pointLightRef.current) {
      if (isThinking) {
        const pulse = (Math.sin(t * THINKING_PULSE_HZ * Math.PI * 2) + 1) / 2;
        pointLightRef.current.intensity = 2.0 + pulse * 1.0;
        pointLightRef.current.distance = 5;
        _lightColor.current.set(config.accentColor);
        pointLightRef.current.color.copy(_lightColor.current);
        pointLightRef.current.visible = true;
      } else if (isWriting) {
        pointLightRef.current.intensity = 1.5;
        pointLightRef.current.distance = 4;
        _lightColor.current.set('#22c55e');
        pointLightRef.current.color.copy(_lightColor.current);
        pointLightRef.current.visible = true;
      } else if (isCompleted && terminalStartRef.current != null) {
        const elapsed = t - terminalStartRef.current;
        if (elapsed < FLASH_DURATION) {
          pointLightRef.current.intensity = 3.0 * (1 - elapsed / FLASH_DURATION);
          pointLightRef.current.distance = 5;
          _lightColor.current.set('#22c55e');
          pointLightRef.current.color.copy(_lightColor.current);
          pointLightRef.current.visible = true;
        } else {
          pointLightRef.current.visible = false;
        }
      } else if (isFailed && terminalStartRef.current != null) {
        const elapsed = t - terminalStartRef.current;
        if (elapsed < FLASH_DURATION) {
          pointLightRef.current.intensity = 2.5 * (1 - elapsed / FLASH_DURATION);
          pointLightRef.current.distance = 4;
          _lightColor.current.set('#ef4444');
          pointLightRef.current.color.copy(_lightColor.current);
          pointLightRef.current.visible = true;
        } else {
          pointLightRef.current.visible = false;
        }
      } else {
        pointLightRef.current.visible = false;
      }
    }

    // ── Orbit ring ──────────────────────────────────────────────────────────

    if (ringRef.current) {
      if (isThinking) {
        // Fast spin during thinking (~2.5 full rotations/sec)
        ringRef.current.rotation.y += dt * Math.PI * 5;
        const mat = ringRef.current.material as THREE.MeshBasicMaterial;
        const pulse = (Math.sin(t * THINKING_PULSE_HZ * Math.PI * 2) + 1) / 2;
        mat.opacity = 0.3 + pulse * 0.4;
        mat.color.set(config.accentColor);
        ringRef.current.visible = true;
      } else if (isWriting) {
        // Slower calm spin
        ringRef.current.rotation.y += dt * Math.PI * 1.5;
        const mat = ringRef.current.material as THREE.MeshBasicMaterial;
        mat.opacity = 0.25;
        mat.color.set('#22c55e');
        ringRef.current.visible = true;
      } else {
        ringRef.current.visible = false;
      }
    }
  });

  // ── JSX ──────────────────────────────────────────────────────────────────

  // Initialize group at the agent's home position. Subsequent positions are
  // driven imperatively in useFrame — we set initial position via group prop
  // and then let the ref take over each frame.
  const idlePos = getAgentIdlePosition(config.agentName);

  // Confidence display string — shown in completed state if present
  const confidenceStr =
    confidence != null ? `${Math.round(confidence * 100)}%` : null;

  return (
    <group
      ref={groupRef}
      position={idlePos}
    >
      {/* Ground shadow — flat transparent ellipse at feet */}
      <mesh
        ref={shadowRef}
        rotation={[-Math.PI / 2, 0, 0]}
        position={[0, 0.01, 0]}
      >
        <circleGeometry args={[0.22, 24]} />
        <meshBasicMaterial
          color="#000000"
          transparent
          opacity={0.18}
          depthWrite={false}
        />
      </mesh>

      {/* Body — capsule humanoid torso+legs */}
      <mesh ref={bodyRef} position={[0, BODY_CENTER_Y, 0]} castShadow>
        {/*
          CapsuleGeometry(radius, height, capSegments, radialSegments)
          height is the straight cylinder portion; total height = height + 2*radius
        */}
        <capsuleGeometry args={[BODY_RADIUS, BODY_HEIGHT, 4, 10]} />
        <meshStandardMaterial
          color={config.color}
          emissive={config.color}
          emissiveIntensity={0.1}
          roughness={0.45}
          metalness={0.35}
          transparent
          opacity={1.0}
        />
      </mesh>

      {/* Head sphere */}
      <mesh ref={headRef} position={[0, HEAD_Y, 0]} castShadow>
        <sphereGeometry args={[HEAD_RADIUS, 16, 12]} />
        <meshStandardMaterial
          color={config.accentColor}
          emissive={config.accentColor}
          emissiveIntensity={0.15}
          roughness={0.5}
          metalness={0.2}
          transparent
          opacity={1.0}
        />
      </mesh>

      {/* Glow aura — BackSide sphere wrapping the avatar, only visible during thinking/writing */}
      <mesh
        ref={auraRef}
        position={[0, AURA_Y, 0]}
        visible={false}
      >
        <sphereGeometry args={[AURA_RADIUS, 24, 16]} />
        <meshBasicMaterial
          color={config.accentColor}
          transparent
          opacity={0.08}
          depthWrite={false}
          side={THREE.BackSide}
        />
      </mesh>

      {/* Orbit ring — horizontal plane, spins during thinking/writing */}
      <mesh
        ref={ringRef}
        position={[0, RING_Y, 0]}
        visible={false}
      >
        <torusGeometry args={[RING_RADIUS, 0.012, 6, 48]} />
        <meshBasicMaterial
          color={config.accentColor}
          transparent
          opacity={0.3}
          depthWrite={false}
        />
      </mesh>

      {/* Point light driven by aura state */}
      <pointLight
        ref={pointLightRef}
        color={config.accentColor}
        intensity={0}
        distance={5}
        decay={2}
        position={[0, LIGHT_Y, 0]}
        visible={false}
      />

      {/* Abbreviation badge above head (Html matches AgentNode/PatientNode pattern) */}
      <Html
        position={[0, BADGE_Y, 0]}
        center
        distanceFactor={18}
        style={{ pointerEvents: 'none' }}
      >
        <div style={{
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          gap: 2,
        }}>
          <span style={{
            background: `${config.color}dd`,
            color: '#ffffff',
            fontSize: 10,
            fontWeight: 800,
            padding: '2px 6px',
            borderRadius: 4,
            letterSpacing: '0.06em',
            boxShadow: `0 0 8px ${config.color}60`,
            whiteSpace: 'nowrap',
          }}>
            {config.abbr}
            {confidenceStr && ` ${confidenceStr}`}
          </span>
        </div>
      </Html>

      {/* Agent label below feet */}
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
          textShadow: '0 1px 3px rgba(0,0,0,0.6)',
          whiteSpace: 'nowrap',
        }}>
          {config.label}
        </span>
      </Html>
    </group>
  );
}
