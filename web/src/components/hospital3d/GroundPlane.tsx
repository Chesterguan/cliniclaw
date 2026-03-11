'use client';

import { useRef } from 'react';
import { useFrame } from '@react-three/fiber';
import * as THREE from 'three';

/**
 * Atmospheric ground plane with concentric rings marking the agent and patient orbits.
 * Values baked from previously leva-tuned defaults.
 */
export function GroundPlane() {
  const innerRingRef = useRef<THREE.Mesh>(null);
  const outerRingRef = useRef<THREE.Mesh>(null);
  const crosshairGroupRef = useRef<THREE.Group>(null);

  const groundColor = '#060810';
  const centerGlowColor = '#1a2040';
  const centerGlowOpacity = 0.4;
  const innerRingColor = '#22d3ee';
  const innerRingOpacity = 0.06;
  const outerRingColor = '#818cf8';
  const outerRingOpacity = 0.04;
  const gridRingOpacity = 0.15;
  const crosshairOpacity = 0.3;
  const rotationSpeed = 0.02;

  useFrame(() => {
    const t = performance.now() / 1000;

    if (crosshairGroupRef.current) {
      crosshairGroupRef.current.rotation.y = t * rotationSpeed;
    }

    if (innerRingRef.current) {
      const mat = innerRingRef.current.material as THREE.MeshBasicMaterial;
      mat.opacity = innerRingOpacity + Math.sin(t * 0.8) * 0.015;
    }
    if (outerRingRef.current) {
      const mat = outerRingRef.current.material as THREE.MeshBasicMaterial;
      mat.opacity = outerRingOpacity + Math.sin(t * 0.6 + 1) * 0.01;
    }
  });

  return (
    <group>
      {/* Main ground disc */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -0.02, 0]} receiveShadow>
        <circleGeometry args={[16, 64]} />
        <meshStandardMaterial
          color={groundColor}
          roughness={0.95}
          metalness={0.05}
        />
      </mesh>

      {/* Center glow spot */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[0, -0.01, 0]}>
        <circleGeometry args={[3, 48]} />
        <meshBasicMaterial
          color={centerGlowColor}
          transparent
          opacity={centerGlowOpacity}
          depthWrite={false}
        />
      </mesh>

      {/* Inner orbit ring (patient ring ~3.8) */}
      <mesh ref={innerRingRef} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.005, 0]}>
        <ringGeometry args={[3.6, 4.0, 96]} />
        <meshBasicMaterial
          color={innerRingColor}
          transparent
          opacity={innerRingOpacity}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Outer orbit ring (agent ring ~9) */}
      <mesh ref={outerRingRef} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.005, 0]}>
        <ringGeometry args={[8.7, 9.3, 96]} />
        <meshBasicMaterial
          color={outerRingColor}
          transparent
          opacity={outerRingOpacity}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Crosshair radial guide lines */}
      <group ref={crosshairGroupRef}>
        {[0, Math.PI / 4, Math.PI / 2, (3 * Math.PI) / 4].map((angle, i) => {
          const x1 = Math.cos(angle) * 14;
          const z1 = Math.sin(angle) * 14;
          const x2 = Math.cos(angle + Math.PI) * 14;
          const z2 = Math.sin(angle + Math.PI) * 14;
          const points = [
            new THREE.Vector3(x1, 0.003, z1),
            new THREE.Vector3(x2, 0.003, z2),
          ];
          const geo = new THREE.BufferGeometry().setFromPoints(points);
          return (
            <primitive
              key={i}
              object={new THREE.Line(
                geo,
                new THREE.LineBasicMaterial({
                  color: '#1e293b',
                  transparent: true,
                  opacity: crosshairOpacity,
                  depthWrite: false,
                }),
              )}
            />
          );
        })}
      </group>

      {/* Concentric grid rings */}
      {[2, 5, 7, 11, 13].map((r) => (
        <mesh key={r} rotation={[-Math.PI / 2, 0, 0]} position={[0, 0.003, 0]}>
          <ringGeometry args={[r - 0.02, r + 0.02, 64]} />
          <meshBasicMaterial
            color="#1e293b"
            transparent
            opacity={gridRingOpacity}
            depthWrite={false}
            side={THREE.DoubleSide}
          />
        </mesh>
      ))}
    </group>
  );
}
