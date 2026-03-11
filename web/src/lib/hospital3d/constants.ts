// 3D Hospital simulation constants — room layout, agent configs, patient placement

// ── Room definitions ────────────────────────────────────────────────────────

export type RoomType = 'exam' | 'triage' | 'nurse_station' | 'lab' | 'pharmacy' | 'discharge';

export interface RoomConfig {
  id: string;
  type: RoomType;
  label: string;
  /** Center of room floor [x, y, z] */
  position: [number, number, number];
  /** [width (X), depth (Z)] in meters */
  size: [number, number];
  /** Floor tint color */
  floorColor: string;
}

/*
 * Hospital floor plan (top-down, Y-up, XZ is floor):
 *
 *   ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐ ┌─────┐
 *   │Exam1│ │Exam2│ │Exam3│ │Exam4│ │Exam5│ │Exam6│     Z = -5.5
 *   └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘ └──┬──┘
 *  ════╪═══════╪═══════╪═══════╪═══════╪═══════╪════    Z = -1.5 to 1.5
 *              M A I N   H A L L W A Y
 *  ════╪═══════╪═══════╪═══════╪═══════╪═══════╪════
 *   ┌──┴──┐       ┌──┴──┐ ┌──┴──┐ ┌──┴──┐ ┌──┴──┐
 *   │Triag│       │Nurse│ │ Lab │ │Pharm│ │Disch│     Z = 5.5
 *   └─────┘       └─────┘ └─────┘ └─────┘ └─────┘
 *
 *   X: -12.5  -7.5  -2.5   2.5   7.5  12.5
 */

const ROOM_W = 3.8;
const ROOM_D = 3.8;
const WALL_H = 2.8;

export const WALL_HEIGHT = WALL_H;

// North row — exam rooms (patients are here)
// South row — department rooms (agent home bases)
export const ROOMS: RoomConfig[] = [
  // Exam rooms (north side, Z = -5.5)
  { id: 'exam-1', type: 'exam',  label: 'Exam 1', position: [-12.5, 0, -5.5], size: [ROOM_W, ROOM_D], floorColor: '#0d1117' },
  { id: 'exam-2', type: 'exam',  label: 'Exam 2', position: [-7.5,  0, -5.5], size: [ROOM_W, ROOM_D], floorColor: '#0d1117' },
  { id: 'exam-3', type: 'exam',  label: 'Exam 3', position: [-2.5,  0, -5.5], size: [ROOM_W, ROOM_D], floorColor: '#0d1117' },
  { id: 'exam-4', type: 'exam',  label: 'Exam 4', position: [2.5,   0, -5.5], size: [ROOM_W, ROOM_D], floorColor: '#0d1117' },
  { id: 'exam-5', type: 'exam',  label: 'Exam 5', position: [7.5,   0, -5.5], size: [ROOM_W, ROOM_D], floorColor: '#0d1117' },
  { id: 'exam-6', type: 'exam',  label: 'Exam 6', position: [12.5,  0, -5.5], size: [ROOM_W, ROOM_D], floorColor: '#0d1117' },

  // Department rooms (south side, Z = 5.5)
  { id: 'triage',        type: 'triage',        label: 'Triage / ER',   position: [-12.5, 0, 5.5], size: [ROOM_W, ROOM_D], floorColor: '#11141e' },
  { id: 'nurse-station', type: 'nurse_station',  label: 'Nurse Station', position: [-5,    0, 5.5], size: [ROOM_W, ROOM_D], floorColor: '#0f1419' },
  { id: 'lab',           type: 'lab',            label: 'Laboratory',    position: [0,     0, 5.5], size: [ROOM_W, ROOM_D], floorColor: '#0f1619' },
  { id: 'pharmacy',      type: 'pharmacy',       label: 'Pharmacy',      position: [5,     0, 5.5], size: [ROOM_W, ROOM_D], floorColor: '#12101a' },
  { id: 'discharge',     type: 'discharge',      label: 'Discharge',     position: [12.5,  0, 5.5], size: [ROOM_W, ROOM_D], floorColor: '#0f1518' },
];

// Hallway bounds
export const HALLWAY = {
  xMin: -15,
  xMax: 15,
  zCenter: 0,
  zHalfWidth: 1.5,  // hallway from Z = -1.5 to Z = 1.5
  color: '#0a0d14',
};

// ── Patient → Room mapping ──────────────────────────────────────────────────

export interface PatientConfig {
  encounterId: string;
  name: string;
  condition: string;
  roomId: string;
  color: string;
}

export const PATIENTS: PatientConfig[] = [
  { encounterId: 'enc-001', name: 'Sarah M.',  condition: 'HTN',      roomId: 'exam-1', color: '#94a3b8' },
  { encounterId: 'enc-002', name: 'James T.',  condition: 'T2DM',     roomId: 'exam-2', color: '#a1a1aa' },
  { encounterId: 'enc-003', name: 'Maria G.',  condition: 'Prenatal', roomId: 'exam-3', color: '#9ca3af' },
  { encounterId: 'enc-004', name: 'Robert C.', condition: 'COPD',     roomId: 'exam-4', color: '#a3a3a3' },
  { encounterId: 'enc-005', name: 'Emily J.',  condition: 'Knee OA',  roomId: 'exam-5', color: '#a8a29e' },
  { encounterId: 'enc-006', name: 'David W.',  condition: 'CHF',      roomId: 'exam-6', color: '#93a5b8' },
];

// Quick lookup: encounterId → roomId
export const ENCOUNTER_ROOM: Record<string, string> = Object.fromEntries(
  PATIENTS.map(p => [p.encounterId, p.roomId])
);

// Quick lookup: roomId → RoomConfig
export const ROOM_MAP: Record<string, RoomConfig> = Object.fromEntries(
  ROOMS.map(r => [r.id, r])
);

// ── Agent config ────────────────────────────────────────────────────────────

export interface AgentConfig {
  agentName: string;
  label: string;
  abbr: string;
  color: string;
  accentColor: string;
  /** Room ID where this agent idles */
  homeRoomId: string;
  /** If true, agent walks to patient room. If false, works from home room. */
  isBedside: boolean;
}

export const AGENTS: Record<string, AgentConfig> = {
  triage_assess: {
    agentName: 'triage_assess',  label: 'Triage',     abbr: 'TR',
    color: '#60a5fa',   accentColor: '#93c5fd',
    homeRoomId: 'triage',        isBedside: true,
  },
  nurse_assess: {
    agentName: 'nurse_assess',   label: 'Nurse',      abbr: 'NA',
    color: '#818cf8',   accentColor: '#a5b4fc',
    homeRoomId: 'nurse-station', isBedside: true,
  },
  ambient_doc: {
    agentName: 'ambient_doc',    label: 'Doctor',     abbr: 'AD',
    color: '#38bdf8',   accentColor: '#7dd3fc',
    homeRoomId: 'nurse-station', isBedside: true,
  },
  order_entry: {
    agentName: 'order_entry',    label: 'Orders',     abbr: 'OE',
    color: '#34d399',   accentColor: '#6ee7b7',
    homeRoomId: 'nurse-station', isBedside: true,
  },
  pharmacy_review: {
    agentName: 'pharmacy_review', label: 'Pharmacy',  abbr: 'PR',
    color: '#a78bfa',   accentColor: '#c4b5fd',
    homeRoomId: 'pharmacy',      isBedside: false,
  },
  lab_review: {
    agentName: 'lab_review',     label: 'Lab',        abbr: 'LR',
    color: '#22d3ee',   accentColor: '#67e8f9',
    homeRoomId: 'lab',           isBedside: false,
  },
  prior_auth: {
    agentName: 'prior_auth',     label: 'Prior Auth', abbr: 'PA',
    color: '#f472b6',   accentColor: '#f9a8d4',
    homeRoomId: 'discharge',     isBedside: true,
  },
  discharge_plan: {
    agentName: 'discharge_plan', label: 'Discharge',  abbr: 'DC',
    color: '#2dd4bf',   accentColor: '#5eead4',
    homeRoomId: 'discharge',     isBedside: true,
  },
};

// ── Avatar state types ──────────────────────────────────────────────────────

export type AvatarState =
  | 'idle'           // standing in home room
  | 'walking'        // moving along waypoints to target room
  | 'arriving'       // entering the room
  | 'working'        // context building, policy checks
  | 'thinking'       // LLM call active — bright glow
  | 'writing'        // FHIR write — green indicator
  | 'completed'      // flash, then return
  | 'failed'         // red flash, then return
  | 'returning';     // walking back to home room

// Keep backward-compat aliases for ControlBar/EventPanel
export const AGENT_AVATARS = AGENTS;
export const AGENT_NODES = AGENTS;
export type ConnectionState = AvatarState;
export interface ActiveConnection {
  id: string;
  agentName: string;
  encounterId: string;
  state: AvatarState;
  confidence: number | null;
  startedAt: number;
}
