'use client';

/**
 * ConnectionBeam — animated beam connecting an agent node to a patient node
 * in the network graph view.
 *
 * Renders a quadratic bezier arc with:
 *   - A line along the full curve (50 sample points, transparent)
 *   - 8 flowing dots (InstancedMesh) traveling along the curve
 *   - A glow tube (TubeGeometry) visible during thinking/writing
 *   - State-driven colors: active=agent color, thinking=accent, writing=green
 *   - completed/failed: fade out over 2.5 s from startedAt
 *
 * All animation is purely in useFrame — no React state updates per frame.
 */

import { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

// ── Props ────────────────────────────────────────────────────────────────────

interface ConnectionBeamProps {
  agentPos: [number, number, number];
  patientPos: [number, number, number];
  color: string;
  accentColor: string;
  /** AvatarState string — driving visual style. */
  state: string;
  /** Seconds (performance.now()/1000) when this connection last changed to a
   *  terminal or newly-tracked state; used to drive the fade-out timer. */
  startedAt: number;
  confidence: number | null;
}

// ── Constants ────────────────────────────────────────────────────────────────

const CURVE_POINTS   = 50;
const DOT_COUNT      = 8;
const DOT_RADIUS     = 0.035;
const TUBE_RADIUS    = 0.025;
const FADE_DURATION  = 2.5;   // seconds for completed/failed fade

// State-specific dot travel speeds (curve parameter units per second).
// The curve parameter runs 0..1; speed × time gives the per-frame offset.
const SPEED_THINKING = 1.5;
const SPEED_WRITING  = 0.8;
const SPEED_DEFAULT  = 0.35;

// Colors
const GREEN = '#22c55e';
const RED   = '#ef4444';

// ── Curve helpers ─────────────────────────────────────────────────────────────

/**
 * Build a quadratic bezier from agentPos to patientPos.
 * The mid-control-point is elevated proportionally to XZ distance so
 * longer connections arc higher, preventing overlap at the centre.
 */
function buildCurve(
  agentPos: [number, number, number],
  patientPos: [number, number, number],
): THREE.QuadraticBezierCurve3 {
  const a = new THREE.Vector3(...agentPos);
  const b = new THREE.Vector3(...patientPos);

  const dx = b.x - a.x;
  const dz = b.z - a.z;
  const xzDist = Math.sqrt(dx * dx + dz * dz);
  const midY = 1.0 + xzDist * 0.12;

  const mid = new THREE.Vector3((a.x + b.x) / 2, midY, (a.z + b.z) / 2);
  return new THREE.QuadraticBezierCurve3(a, mid, b);
}

// ── Component ─────────────────────────────────────────────────────────────────

export function ConnectionBeam({
  agentPos,
  patientPos,
  color,
  accentColor,
  state,
  startedAt,
}: ConnectionBeamProps) {
  // ── Mesh refs ─────────────────────────────────────────────────────────────
  const dotsRef = useRef<THREE.InstancedMesh>(null);
  const tubeRef = useRef<THREE.Mesh>(null);

  // ── Pre-allocated per-frame scratch objects ───────────────────────────────
  const _matrix = useMemo(() => new THREE.Matrix4(), []);
  const _pos    = useMemo(() => new THREE.Vector3(), []);
  const _scale  = useMemo(() => new THREE.Vector3(), []);
  const _quat   = useMemo(() => new THREE.Quaternion(), []);

  // ── Stable curve — only recomputed when positions actually change ─────────

  const curve = useMemo(
    () => buildCurve(agentPos, patientPos),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      agentPos[0], agentPos[1], agentPos[2],
      patientPos[0], patientPos[1], patientPos[2],
    ],
  );

  // Sampled curve points for dot positioning (flat array, fast index lookup)
  const curvePoints = useMemo(() => curve.getPoints(CURVE_POINTS - 1), [curve]);

  // ── Line object — stable THREE.Line created once per connection ───────────

  const lineObject = useMemo(() => {
    const pts = curve.getPoints(CURVE_POINTS - 1);
    const geo = new THREE.BufferGeometry().setFromPoints(pts);
    const mat = new THREE.LineBasicMaterial({
      color,
      transparent: true,
      opacity: 0.3,
      depthWrite: false,
    });
    return new THREE.Line(geo, mat);
  }, [curve, color]); // color only used for initial creation; animated below

  // ── TubeGeometry along the same curve for the glow tube ──────────────────

  const tubeGeometry = useMemo(
    () => new THREE.TubeGeometry(curve, 32, TUBE_RADIUS, 6, false),
    [curve],
  );

  // ── Animation ─────────────────────────────────────────────────────────────

  useFrame(() => {
    const t = performance.now() / 1000;

    const isThinking  = state === 'thinking';
    const isWriting   = state === 'writing';
    const isCompleted = state === 'completed';
    const isFailed    = state === 'failed';
    const isTerminal  = isCompleted || isFailed;

    // Fade factor for completed/failed: 1.0 → 0.0 over FADE_DURATION seconds
    let fadeFactor = 1.0;
    if (isTerminal) {
      const elapsed = t - startedAt;
      fadeFactor = Math.max(0, 1 - elapsed / FADE_DURATION);
    }

    // Resolve display color for this frame
    let beamColor: string;
    if (isCompleted)      beamColor = GREEN;
    else if (isFailed)    beamColor = RED;
    else if (isThinking)  beamColor = accentColor;
    else if (isWriting)   beamColor = GREEN;
    else                  beamColor = color;

    // Dot travel speed and bidirectional flag
    let speed = SPEED_DEFAULT;
    let bidir = false;
    if (isThinking)      { speed = SPEED_THINKING; bidir = true; }
    else if (isWriting)  { speed = SPEED_WRITING; }

    const visible = fadeFactor > 0.01;

    // ── Arc line ──────────────────────────────────────────────────────────

    lineObject.visible = visible;
    if (visible) {
      const mat = lineObject.material as THREE.LineBasicMaterial;
      mat.color.set(beamColor);
      mat.opacity = (isThinking ? 0.55 : isWriting ? 0.45 : 0.3) * fadeFactor;
    }

    // ── Flowing dots ──────────────────────────────────────────────────────

    if (dotsRef.current) {
      dotsRef.current.visible = visible;

      if (visible) {
        const ptCount = curvePoints.length;

        for (let i = 0; i < DOT_COUNT; i++) {
          const phase = i / DOT_COUNT;

          // Bidirectional: odd-indexed dots travel in the reverse direction
          let tParam: number;
          if (bidir && i % 2 === 1) {
            tParam = 1.0 - ((phase + t * speed) % 1.0);
          } else {
            tParam = (phase + t * speed) % 1.0;
          }

          // Nearest sample index from pre-computed curve points
          const sampleIdx = Math.min(
            Math.floor(tParam * (ptCount - 1)),
            ptCount - 1,
          );
          const pt = curvePoints[sampleIdx];
          _pos.set(pt.x, pt.y, pt.z);

          // Dots appear largest at arc midpoint — gives depth impression
          const midness  = 1.0 - Math.abs(tParam - 0.5) * 2;
          const dotScale = (0.6 + midness * 0.8) * fadeFactor;

          _scale.setScalar(dotScale);
          _matrix.compose(_pos, _quat, _scale);
          dotsRef.current.setMatrixAt(i, _matrix);
        }
        dotsRef.current.instanceMatrix.needsUpdate = true;

        const mat = dotsRef.current.material as THREE.MeshBasicMaterial;
        mat.color.set(beamColor);
        mat.opacity = (isThinking ? 0.9 : isWriting ? 0.8 : 0.65) * fadeFactor;
      }
    }

    // ── Glow tube ─────────────────────────────────────────────────────────

    if (tubeRef.current) {
      const showTube = (isThinking || isWriting) && visible;
      tubeRef.current.visible = showTube;

      if (showTube) {
        const mat = tubeRef.current.material as THREE.MeshBasicMaterial;
        mat.color.set(beamColor);
        const pulse = isThinking
          ? (Math.sin(t * 4.5 * Math.PI * 2) + 1) / 2
          : (Math.sin(t * 2.0 * Math.PI * 2) + 1) / 2;
        mat.opacity = (0.08 + pulse * 0.12) * fadeFactor;
      }
    }
  });

  // ── JSX ───────────────────────────────────────────────────────────────────

  return (
    <group>
      {/* ── Arc line (stable primitive, material animated in useFrame) ─── */}
      <primitive object={lineObject} />

      {/* ── Flowing dots (InstancedMesh of small spheres) ──────────────── */}
      <instancedMesh
        ref={dotsRef}
        args={[undefined, undefined, DOT_COUNT]}
        frustumCulled={false}
      >
        <sphereGeometry args={[DOT_RADIUS, 6, 6]} />
        <meshBasicMaterial
          color={color}
          transparent
          opacity={0.65}
          depthWrite={false}
        />
      </instancedMesh>

      {/* ── Glow tube — visible only during thinking/writing ───────────── */}
      <mesh ref={tubeRef} geometry={tubeGeometry} visible={false}>
        <meshBasicMaterial
          color={accentColor}
          transparent
          opacity={0.1}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>
    </group>
  );
}
