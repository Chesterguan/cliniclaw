'use client';

/**
 * HospitalFloor — renders the complete hospital floor plan.
 *
 * Includes:
 *   - All rooms from ROOMS constant via Room component
 *   - Main hallway corridor plane
 *   - Thin connector strips from each room door to hallway edge
 *   - Center divider line on hallway
 *   - Large ground disc
 *   - Subtle dashed walkway markers on hallway centerline
 */

import { useMemo } from 'react';
import * as THREE from 'three';
import { ROOMS, HALLWAY, ROOM_MAP } from '@/lib/hospital3d/constants';
import { Room } from './Room';

interface HospitalFloorProps {
  /** Maps roomId → agent hex color for rooms currently hosting an active agent */
  activeRooms?: Map<string, string>;
}

// Hallway Z extents
const HALL_Z_MIN = HALLWAY.zCenter - HALLWAY.zHalfWidth; // -1.5
const HALL_Z_MAX = HALLWAY.zCenter + HALLWAY.zHalfWidth; //  1.5
const HALL_WIDTH = HALL_Z_MAX - HALL_Z_MIN;               //  3.0
const HALL_LENGTH = HALLWAY.xMax - HALLWAY.xMin;          // 30.0
const HALL_X_CENTER = (HALLWAY.xMin + HALLWAY.xMax) / 2;  //  0

// Y offsets — everything slightly above ground to avoid Z-fighting
const Y_GROUND = -0.01;
const Y_HALLWAY = 0.005;
const Y_CONNECTOR = 0.006;
const Y_CENTER_LINE = 0.008;
const Y_DASHES = 0.009;

export function HospitalFloor({ activeRooms }: HospitalFloorProps) {
  // Build connector strip geometry for each room door → hallway edge (imperative)
  const connectorStrips = useMemo(() => {
    const strips: { key: string; x: number; zFrom: number; zTo: number; depth: number }[] = [];

    for (const room of ROOMS) {
      const [rx, , rz] = room.position;
      const halfD = room.size[1] / 2;

      if (rz < 0) {
        // North room: connector from south face of room to north hallway edge
        const doorZ = rz + halfD;   // south face of room
        const hallEdgeZ = HALL_Z_MIN; // -1.5
        const connectorDepth = hallEdgeZ - doorZ; // negative if doorZ > hallEdgeZ (they don't overlap)
        // doorZ = -5.5 + 1.9 = -3.6, hallEdgeZ = -1.5 → depth = 2.1
        const depth = Math.abs(connectorDepth);
        const midZ = (doorZ + hallEdgeZ) / 2;
        strips.push({ key: room.id, x: rx, zFrom: doorZ, zTo: hallEdgeZ, depth, midZ } as typeof strips[0] & { midZ: number });
      } else {
        // South room: connector from north face of room to south hallway edge
        const doorZ = rz - halfD;   // north face of room
        const hallEdgeZ = HALL_Z_MAX; // 1.5
        const depth = Math.abs(doorZ - hallEdgeZ);
        const midZ = (doorZ + hallEdgeZ) / 2;
        strips.push({ key: room.id, x: rx, zFrom: hallEdgeZ, zTo: doorZ, depth, midZ } as typeof strips[0] & { midZ: number });
      }
    }
    return strips;
  }, []);

  // Dashed walkway markers — short thin boxes spaced along hallway center
  const dashMarkers = useMemo(() => {
    const dashes: number[] = [];
    const dashSpacing = 2.5;
    const start = HALLWAY.xMin + 1.0;
    const end = HALLWAY.xMax - 1.0;
    for (let x = start; x <= end; x += dashSpacing) {
      dashes.push(x);
    }
    return dashes;
  }, []);

  return (
    <group>
      {/* ── Ground circle ───────────────────────────────────────────── */}
      <mesh rotation={[-Math.PI / 2, 0, 0]} position={[HALL_X_CENTER, Y_GROUND, 0]} receiveShadow>
        <circleGeometry args={[25, 64]} />
        <meshStandardMaterial color="#050508" roughness={0.98} metalness={0.0} />
      </mesh>

      {/* ── Main hallway plane ──────────────────────────────────────── */}
      <mesh
        rotation={[-Math.PI / 2, 0, 0]}
        position={[HALL_X_CENTER, Y_HALLWAY, HALLWAY.zCenter]}
        receiveShadow
      >
        <planeGeometry args={[HALL_LENGTH, HALL_WIDTH]} />
        <meshStandardMaterial
          color="#0c1018"
          roughness={0.9}
          metalness={0.05}
        />
      </mesh>

      {/* ── Hallway center divider line ─────────────────────────────── */}
      <mesh
        rotation={[-Math.PI / 2, 0, 0]}
        position={[HALL_X_CENTER, Y_CENTER_LINE, 0]}
      >
        <planeGeometry args={[HALL_LENGTH, 0.04]} />
        <meshBasicMaterial
          color="#1e2a3a"
          transparent
          opacity={0.02}
          depthWrite={false}
          side={THREE.DoubleSide}
        />
      </mesh>

      {/* ── Dashed walkway markers ──────────────────────────────────── */}
      {dashMarkers.map((x) => (
        <mesh
          key={`dash-${x}`}
          rotation={[-Math.PI / 2, 0, 0]}
          position={[x, Y_DASHES, 0]}
        >
          <planeGeometry args={[0.8, 0.06]} />
          <meshBasicMaterial
            color="#1e3a4a"
            transparent
            opacity={0.12}
            depthWrite={false}
            side={THREE.DoubleSide}
          />
        </mesh>
      ))}

      {/* ── Room connector strips ───────────────────────────────────── */}
      {connectorStrips.map((strip) => {
        const midZ = (strip.zFrom + strip.zTo) / 2;
        return (
          <mesh
            key={`conn-${strip.key}`}
            rotation={[-Math.PI / 2, 0, 0]}
            position={[strip.x, Y_CONNECTOR, midZ]}
          >
            <planeGeometry args={[0.8, strip.depth]} />
            <meshStandardMaterial
              color="#0b0f15"
              roughness={0.95}
              metalness={0.0}
            />
          </mesh>
        );
      })}

      {/* ── Room geometry ───────────────────────────────────────────── */}
      {ROOMS.map((room) => (
        <Room
          key={room.id}
          config={room}
          isActive={activeRooms?.has(room.id) ?? false}
          activeAgentColor={activeRooms?.get(room.id)}
        />
      ))}
    </group>
  );
}
