'use client';

import { OrbitControls } from '@react-three/drei';
import { useThree, useFrame } from '@react-three/fiber';
import { useRef, useEffect } from 'react';
import * as THREE from 'three';

export type CameraPreset = 'overview' | 'top-down' | 'close-up';

interface CameraRigProps {
  preset: CameraPreset;
  followTarget?: [number, number, number] | null;
}

const PRESETS: Record<string, { position: THREE.Vector3; target: THREE.Vector3 }> = {
  overview: {
    position: new THREE.Vector3(0, 30, 22),
    target: new THREE.Vector3(0, 0, 0),
  },
  'top-down': {
    position: new THREE.Vector3(0, 42, 0.1),
    target: new THREE.Vector3(0, 0, 0),
  },
  'close-up': {
    position: new THREE.Vector3(8, 12, 12),
    target: new THREE.Vector3(0, 0, 0),
  },
};

export function CameraRig({ preset }: CameraRigProps) {
  const { camera } = useThree();
  const controlsRef = useRef<any>(null);
  const targetPos = useRef(new THREE.Vector3(0, 30, 22));
  const targetLookAt = useRef(new THREE.Vector3(0, 0, 0));
  const isTransitioning = useRef(false);

  useEffect(() => {
    const p = PRESETS[preset] ?? PRESETS.overview;
    targetPos.current.copy(p.position);
    targetLookAt.current.copy(p.target);
    isTransitioning.current = true;
  }, [preset]);

  useFrame(() => {
    if (!isTransitioning.current) return;

    camera.position.lerp(targetPos.current, 0.04);
    if (controlsRef.current) {
      controlsRef.current.target.lerp(targetLookAt.current, 0.04);
      controlsRef.current.update();
    }

    const dist = camera.position.distanceTo(targetPos.current);
    if (dist < 0.1) {
      isTransitioning.current = false;
    }
  });

  return (
    <OrbitControls
      ref={controlsRef}
      minPolarAngle={Math.PI / 8}
      maxPolarAngle={Math.PI / 2.2}
      enableDamping
      dampingFactor={0.05}
      minDistance={5}
      maxDistance={60}
    />
  );
}
