// Hospital spatial layout — pathfinding waypoints and position helpers

import { ROOM_MAP, HALLWAY, AGENTS, ENCOUNTER_ROOM } from './constants';

type Vec3 = [number, number, number];

const AVATAR_Y = 0.0; // avatars walk at floor level

/**
 * Get the "door" position of a room — the point where the room opens onto the hallway.
 * North rooms (Z < 0) have doors on their south face.
 * South rooms (Z > 0) have doors on their north face.
 */
export function getRoomDoor(roomId: string): Vec3 {
  const room = ROOM_MAP[roomId];
  if (!room) return [0, AVATAR_Y, 0];
  const [, , rz] = room.position;
  const halfD = room.size[1] / 2;

  if (rz < 0) {
    return [room.position[0], AVATAR_Y, rz + halfD + 0.3];
  } else {
    return [room.position[0], AVATAR_Y, rz - halfD - 0.3];
  }
}

/**
 * Get the "bedside" position inside a room — where the agent stands to work.
 * Slightly offset from room center so multiple agents don't stack.
 */
export function getRoomBedside(roomId: string, agentIndex: number = 0): Vec3 {
  const room = ROOM_MAP[roomId];
  if (!room) return [0, AVATAR_Y, 0];
  const xOffset = (agentIndex % 3 - 1) * 0.6;
  return [room.position[0] + xOffset, AVATAR_Y, room.position[2]];
}

/**
 * Get the idle position for an agent in their home room.
 * Multiple agents in the same home room get different offsets.
 */
export function getAgentIdlePosition(agentName: string): Vec3 {
  const agent = AGENTS[agentName];
  if (!agent) return [0, AVATAR_Y, 0];
  const room = ROOM_MAP[agent.homeRoomId];
  if (!room) return [0, AVATAR_Y, 0];

  const roommates = Object.values(AGENTS).filter(a => a.homeRoomId === agent.homeRoomId);
  const idx = roommates.findIndex(a => a.agentName === agentName);
  const xOff = (idx % 3 - 1) * 0.8;
  const zOff = idx >= 3 ? 0.8 : 0;

  return [room.position[0] + xOff, AVATAR_Y, room.position[2] + zOff];
}

/**
 * Build a waypoint path from one room to another through the hallway.
 * Path: home center → home door → hallway Z=0 → target door → target bedside
 */
export function buildPath(fromRoomId: string, toRoomId: string, agentName: string): Vec3[] {
  if (fromRoomId === toRoomId) {
    const agentIdx = getAgentIndexInRoom(agentName);
    return [getRoomBedside(toRoomId, agentIdx)];
  }

  const fromDoor = getRoomDoor(fromRoomId);
  const toDoor = getRoomDoor(toRoomId);
  const agentIdx = getAgentIndexInRoom(agentName);

  const path: Vec3[] = [];

  path.push(fromDoor);
  path.push([fromDoor[0], AVATAR_Y, HALLWAY.zCenter]);

  if (Math.abs(fromDoor[0] - toDoor[0]) > 0.5) {
    path.push([toDoor[0], AVATAR_Y, HALLWAY.zCenter]);
  }

  path.push(toDoor);
  path.push(getRoomBedside(toRoomId, agentIdx));

  return path;
}

/**
 * Build the return path from a target room back to the agent's home room.
 */
export function buildReturnPath(agentName: string, fromRoomId: string): Vec3[] {
  const agent = AGENTS[agentName];
  if (!agent) return [getAgentIdlePosition(agentName)];

  if (fromRoomId === agent.homeRoomId) {
    return [getAgentIdlePosition(agentName)];
  }

  const fromDoor = getRoomDoor(fromRoomId);
  const toDoor = getRoomDoor(agent.homeRoomId);

  const path: Vec3[] = [];
  path.push(fromDoor);
  path.push([fromDoor[0], AVATAR_Y, HALLWAY.zCenter]);
  if (Math.abs(fromDoor[0] - toDoor[0]) > 0.5) {
    path.push([toDoor[0], AVATAR_Y, HALLWAY.zCenter]);
  }
  path.push(toDoor);
  path.push(getAgentIdlePosition(agentName));

  return path;
}

/**
 * Get the target room for an agent working on an encounter.
 * Bedside agents go to the patient's exam room.
 * Facility agents stay in their home room.
 */
export function getTargetRoom(agentName: string, encounterId: string): string {
  const agent = AGENTS[agentName];
  if (!agent) return 'exam-1';

  if (agent.isBedside) {
    return ENCOUNTER_ROOM[encounterId] || 'exam-1';
  } else {
    return agent.homeRoomId;
  }
}

function getAgentIndexInRoom(agentName: string): number {
  const allAgents = Object.keys(AGENTS);
  return allAgents.indexOf(agentName) % 3;
}

/** Walking speed in units per second */
export const WALK_SPEED = 3.5;

/** Time to linger after completed/failed before returning (ms) */
export const LINGER_AFTER_DONE = 1500;
