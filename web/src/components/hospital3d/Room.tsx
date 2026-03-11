'use client';

/**
 * Room — single hospital room geometry with semi-transparent glass walls.
 *
 * Three walls (back + two sides) are open on the hallway-facing front.
 * North rooms (Z < 0) open south; south rooms (Z > 0) open north.
 * When active, floor emissive pulses and wall edges emit the agent color.
 */

import { useRef, useMemo } from 'react';
import { useFrame } from '@react-three/fiber';
import { Text } from '@react-three/drei';
import * as THREE from 'three';
import { WALL_HEIGHT, type RoomConfig } from '@/lib/hospital3d/constants';

interface RoomProps {
  config: RoomConfig;
  isActive?: boolean;
  activeAgentColor?: string;
}

// Wall geometry constants
const WALL_THICKNESS = 0.06;
const FLOOR_THICKNESS = 0.02;

export function Room({ config, isActive = false, activeAgentColor = '#60a5fa' }: RoomProps) {
  const floorRef = useRef<THREE.Mesh>(null);
  // Edge glow lines on wall perimeters
  const edgeGlowRef = useRef<THREE.Group>(null);

  const [roomW, roomD] = config.size;
  const halfW = roomW / 2;
  const halfD = roomD / 2;

  // North rooms (Z < 0) open south; south rooms (Z > 0) open north.
  const isNorth = config.position[2] < 0;

  // Back wall Z offset from room center: pushed to the closed end
  const backWallZ = isNorth ? -halfD : halfD;
  // Back wall faces inward — north rooms back wall faces +Z, south rooms -Z
  // No rotation needed: a box at this position with default orientation works

  // Wall heights — centered at half the wall height so base sits on Y=0
  const wallCenterY = WALL_HEIGHT / 2;

  // Floor color parsed for emissive use
  const floorColor = useMemo(() => new THREE.Color(config.floorColor), [config.floorColor]);
  const agentColorObj = useMemo(() => new THREE.Color(activeAgentColor), [activeAgentColor]);

  useFrame(() => {
    const t = performance.now() / 1000;

    // Floor emissive pulse when active
    if (floorRef.current) {
      const mat = floorRef.current.material as THREE.MeshStandardMaterial;
      if (isActive) {
        const pulse = Math.sin(t * 1.8) * 0.5 + 0.5; // 0 to 1
        mat.emissiveIntensity = 0.04 + pulse * 0.06;
        mat.emissive.copy(agentColorObj);
      } else {
        mat.emissiveIntensity = 0.0;
      }
    }

    // Wall edge glow opacity
    if (edgeGlowRef.current) {
      const pulse = Math.sin(t * 2.2) * 0.5 + 0.5;
      edgeGlowRef.current.children.forEach((child) => {
        const line = child as THREE.Line;
        if (line.material) {
          const mat = line.material as THREE.LineBasicMaterial;
          mat.opacity = isActive ? 0.25 + pulse * 0.25 : 0.0;
          mat.color.set(isActive ? agentColorObj : new THREE.Color(0x000000));
        }
      });
    }
  });

  // Build edge glow lines for the 3 walls imperatively — avoids SVG <line> collision
  const edgeLines = useMemo(() => {
    const lines: THREE.Line[] = [];

    const makeLineMat = () => new THREE.LineBasicMaterial({
      color: activeAgentColor,
      transparent: true,
      opacity: 0.0,
      depthWrite: false,
    });

    // Helper: build a box outline (4 vertical edges + 4 horizontal edges)
    const addBoxEdges = (
      cx: number,
      cz: number,
      width: number,
      depth: number,
      height: number,
    ) => {
      const hw = width / 2;
      const hd = depth / 2;
      const bottom = 0;
      const top = height;

      // 4 vertical edge lines
      [
        [cx - hw, cz - hd],
        [cx + hw, cz - hd],
        [cx - hw, cz + hd],
        [cx + hw, cz + hd],
      ].forEach(([x, z]) => {
        const geo = new THREE.BufferGeometry().setFromPoints([
          new THREE.Vector3(x, bottom, z),
          new THREE.Vector3(x, top, z),
        ]);
        lines.push(new THREE.Line(geo, makeLineMat()));
      });

      // 4 top horizontal edges
      const topEdges: [THREE.Vector3, THREE.Vector3][] = [
        [new THREE.Vector3(cx - hw, top, cz - hd), new THREE.Vector3(cx + hw, top, cz - hd)],
        [new THREE.Vector3(cx + hw, top, cz - hd), new THREE.Vector3(cx + hw, top, cz + hd)],
        [new THREE.Vector3(cx + hw, top, cz + hd), new THREE.Vector3(cx - hw, top, cz + hd)],
        [new THREE.Vector3(cx - hw, top, cz + hd), new THREE.Vector3(cx - hw, top, cz - hd)],
      ];
      topEdges.forEach(([a, b]) => {
        const geo = new THREE.BufferGeometry().setFromPoints([a, b]);
        lines.push(new THREE.Line(geo, makeLineMat()));
      });
    };

    // Back wall edges (full width × WALL_THICKNESS × WALL_HEIGHT)
    addBoxEdges(0, backWallZ, roomW, WALL_THICKNESS, WALL_HEIGHT);

    // Left wall edges (WALL_THICKNESS × roomD × WALL_HEIGHT), centered at x=-halfW, z=0
    addBoxEdges(-halfW, 0, WALL_THICKNESS, roomD, WALL_HEIGHT);

    // Right wall edges
    addBoxEdges(halfW, 0, WALL_THICKNESS, roomD, WALL_HEIGHT);

    return lines;
  // We intentionally only rebuild when layout-defining values change.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [backWallZ, halfW, roomD, roomW]);

  return (
    <group position={config.position}>
      {/* Floor plane */}
      <mesh
        ref={floorRef}
        position={[0, FLOOR_THICKNESS / 2, 0]}
        receiveShadow
      >
        <boxGeometry args={[roomW, FLOOR_THICKNESS, roomD]} />
        <meshStandardMaterial
          color={config.floorColor}
          emissive={floorColor}
          emissiveIntensity={0.0}
          roughness={0.85}
          metalness={0.05}
        />
      </mesh>

      {/* Back wall — closed end of the room */}
      <mesh
        position={[0, wallCenterY, backWallZ]}
      >
        <boxGeometry args={[roomW, WALL_HEIGHT, WALL_THICKNESS]} />
        <meshPhysicalMaterial
          color="#0d1117"
          transmission={0.85}
          roughness={0.3}
          metalness={0.1}
          thickness={0.5}
          transparent
          opacity={0.55}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Left side wall */}
      <mesh
        position={[-halfW, wallCenterY, 0]}
      >
        <boxGeometry args={[WALL_THICKNESS, WALL_HEIGHT, roomD]} />
        <meshPhysicalMaterial
          color="#0d1117"
          transmission={0.85}
          roughness={0.3}
          metalness={0.1}
          thickness={0.5}
          transparent
          opacity={0.55}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Right side wall */}
      <mesh
        position={[halfW, wallCenterY, 0]}
      >
        <boxGeometry args={[WALL_THICKNESS, WALL_HEIGHT, roomD]} />
        <meshPhysicalMaterial
          color="#0d1117"
          transmission={0.85}
          roughness={0.3}
          metalness={0.1}
          thickness={0.5}
          transparent
          opacity={0.55}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* Edge glow lines — imperative THREE.Line objects */}
      <group ref={edgeGlowRef}>
        {edgeLines.map((line, i) => (
          <primitive key={i} object={line} />
        ))}
      </group>

      {/* Active room point light — subtle fill from inside */}
      {isActive && (
        <pointLight
          position={[0, 1.0, 0]}
          color={activeAgentColor}
          intensity={0.3}
          distance={roomW * 1.5}
          decay={2}
        />
      )}

      {/* Room label — always faces camera via Text from drei */}
      <Text
        position={[0, WALL_HEIGHT + 0.4, 0]}
        fontSize={0.35}
        color="#475569"
        anchorX="center"
        anchorY="middle"
        renderOrder={1}
        depthOffset={1}
      >
        {config.label}
      </Text>
    </group>
  );
}
