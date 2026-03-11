// Zustand store for spatial avatar state in the 3D hospital simulation.
// R3F components read via getState() inside useFrame — zero React re-renders on
// per-frame reads. React components that need to re-render (HospitalScene) use
// the normal zustand selector hook.

import { create } from 'zustand';
import type { AvatarState } from '@/lib/hospital3d/constants';
import { AGENTS } from '@/lib/hospital3d/constants';
import type { AvatarCommand } from '@/lib/hospital3d/avatar-state-machine';
import {
  buildPath,
  buildReturnPath,
  getTargetRoom,
  getAgentIdlePosition,
  LINGER_AFTER_DONE,
} from '@/lib/hospital3d/layout';

// ── Types ────────────────────────────────────────────────────────────────────

export interface AvatarInstance {
  /** Composite key: "agentName:encounterId" */
  id: string;
  agentName: string;
  encounterId: string;
  state: AvatarState;
  /** Room the avatar is currently moving toward or working in */
  targetRoomId: string;
  /** Ordered list of world-space positions along the current movement path */
  waypoints: [number, number, number][];
  confidence: number | null;
  stateEnteredAt: number;
}

interface SceneState {
  /** Active avatars keyed by "agentName:encounterId" */
  avatars: Map<string, AvatarInstance>;
  /** Human clinician review indicators keyed by turnId */
  clinicians: Map<string, { encounterId: string; spawnedAt: number }>;
  simRunning: boolean;

  processCommand(cmd: AvatarCommand): void;
  spawnClinician(turnId: string, encounterId: string): void;
  setSimRunning(running: boolean): void;
  reset(): void;
}

// ── Helpers ──────────────────────────────────────────────────────────────────

function avatarKey(agentName: string, encounterId: string): string {
  return `${agentName}:${encounterId}`;
}

// ── Store ────────────────────────────────────────────────────────────────────

export const useSceneState = create<SceneState>((set, get) => ({
  avatars: new Map(),
  clinicians: new Map(),
  simRunning: false,

  processCommand: (cmd: AvatarCommand) => {
    const { agentName, encounterId, state, confidence } = cmd;
    const key = avatarKey(agentName, encounterId);
    const now = Date.now();
    const current = get().avatars.get(key);

    // ── 'walking': agent just started — create a new avatar with full path ──
    if (state === 'walking') {
      const agentConfig = AGENTS[agentName];
      if (!agentConfig) return;

      const homeRoomId = agentConfig.homeRoomId;
      const targetRoomId = getTargetRoom(agentName, encounterId);
      const waypoints = buildPath(homeRoomId, targetRoomId, agentName);

      const avatar: AvatarInstance = {
        id: key,
        agentName,
        encounterId,
        state: 'walking',
        targetRoomId,
        waypoints,
        confidence: null,
        stateEnteredAt: now,
      };

      const next = new Map(get().avatars);
      next.set(key, avatar);
      set({ avatars: next });
      return;
    }

    // ── 'completed' / 'failed': flash, linger, then start return walk ──
    if (state === 'completed' || state === 'failed') {
      if (!current) return;

      // Mark terminal state immediately
      const updated: AvatarInstance = {
        ...current,
        state,
        confidence: confidence ?? current.confidence,
        stateEnteredAt: now,
      };
      const next1 = new Map(get().avatars);
      next1.set(key, updated);
      set({ avatars: next1 });

      // After linger delay, begin return walk
      setTimeout(() => {
        const live = get().avatars.get(key);
        if (!live) return;

        const returnWaypoints = buildReturnPath(agentName, live.targetRoomId);
        const returning: AvatarInstance = {
          ...live,
          state: 'returning',
          waypoints: returnWaypoints,
          stateEnteredAt: Date.now(),
        };
        const next2 = new Map(get().avatars);
        next2.set(key, returning);
        set({ avatars: next2 });

        // Remove avatar after return walk completes (~3 s at WALK_SPEED)
        setTimeout(() => {
          const final = new Map(get().avatars);
          final.delete(key);
          set({ avatars: final });
        }, 3000);
      }, LINGER_AFTER_DONE);

      return;
    }

    // ── All other states: update existing avatar in place ──
    // ('working', 'thinking', 'writing', 'arriving')
    if (!current) {
      // Guard: if there is no existing avatar we have no position/path context.
      // This can happen if an event fires before agent_started — just ignore.
      return;
    }

    const updated: AvatarInstance = {
      ...current,
      state,
      confidence: confidence ?? current.confidence,
      stateEnteredAt: now,
    };
    const next = new Map(get().avatars);
    next.set(key, updated);
    set({ avatars: next });
  },

  spawnClinician: (turnId: string, encounterId: string) => {
    const clinicians = new Map(get().clinicians);
    clinicians.set(turnId, { encounterId, spawnedAt: Date.now() });
    set({ clinicians });

    // Auto-remove after 5 s — clinician indicator is transient
    setTimeout(() => {
      const c = new Map(get().clinicians);
      c.delete(turnId);
      set({ clinicians: c });
    }, 5000);
  },

  setSimRunning: (running: boolean) => set({ simRunning: running }),

  reset: () =>
    set({
      avatars: new Map(),
      clinicians: new Map(),
      simRunning: false,
    }),
}));

// ── Convenience selectors ────────────────────────────────────────────────────

/** Returns the idle start position for any avatar before its waypoints are set */
export function getIdlePosition(agentName: string): [number, number, number] {
  return getAgentIdlePosition(agentName);
}
