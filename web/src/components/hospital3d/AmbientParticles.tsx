'use client';

import { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

const MAX_PARTICLES = 200;
const PARTICLE_COUNT = 120;
const FIELD_RADIUS = 16;
const FIELD_HEIGHT = 8;
const PARTICLE_COLOR = '#4488aa';
const PARTICLE_OPACITY = 0.3;
const PARTICLE_SIZE = 0.02;
const DRIFT_SPEED = 1.0;

/**
 * Slowly drifting ambient particles that give the scene depth and atmosphere.
 * Values baked from previously leva-tuned defaults.
 */
export function AmbientParticles() {
  const meshRef = useRef<THREE.InstancedMesh>(null);
  const tempMatrix = useMemo(() => new THREE.Matrix4(), []);

  const particleData = useMemo(() => {
    const data: Array<{
      angle: number; radius: number; heightFrac: number;
      speedX: number; speedY: number; speedZ: number;
      phase: number; scaleFrac: number;
    }> = [];
    for (let i = 0; i < MAX_PARTICLES; i++) {
      data.push({
        angle: Math.random() * Math.PI * 2,
        radius: Math.random(),
        heightFrac: Math.random(),
        speedX: (Math.random() - 0.5) * 0.3,
        speedY: (Math.random() - 0.5) * 0.15,
        speedZ: (Math.random() - 0.5) * 0.3,
        phase: Math.random() * Math.PI * 2,
        scaleFrac: 0.3 + Math.random() * 0.7,
      });
    }
    return data;
  }, []);

  useFrame(() => {
    if (!meshRef.current) return;
    const t = performance.now() / 1000;

    const zeroMatrix = new THREE.Matrix4().makeScale(0, 0, 0);

    for (let i = 0; i < MAX_PARTICLES; i++) {
      if (i >= PARTICLE_COUNT) {
        meshRef.current.setMatrixAt(i, zeroMatrix);
        continue;
      }

      const p = particleData[i];
      const r = p.radius * FIELD_RADIUS;
      const baseX = Math.cos(p.angle) * r;
      const baseZ = Math.sin(p.angle) * r;
      const baseY = p.heightFrac * FIELD_HEIGHT - 0.5;

      const x = baseX + Math.sin(t * p.speedX * DRIFT_SPEED + p.phase) * 1.5;
      const y = baseY + Math.sin(t * p.speedY * DRIFT_SPEED + p.phase * 2) * 0.8;
      const z = baseZ + Math.cos(t * p.speedZ * DRIFT_SPEED + p.phase) * 1.5;

      const scale = p.scaleFrac * (0.8 + Math.sin(t * 0.5 + p.phase) * 0.2);

      tempMatrix.makeScale(scale, scale, scale);
      tempMatrix.setPosition(x, y, z);
      meshRef.current.setMatrixAt(i, tempMatrix);
    }
    meshRef.current.instanceMatrix.needsUpdate = true;
  });

  return (
    <instancedMesh ref={meshRef} args={[undefined, undefined, MAX_PARTICLES]} frustumCulled={false}>
      <sphereGeometry args={[PARTICLE_SIZE, 6, 6]} />
      <meshBasicMaterial
        color={PARTICLE_COLOR}
        transparent
        opacity={PARTICLE_OPACITY}
        depthWrite={false}
      />
    </instancedMesh>
  );
}
