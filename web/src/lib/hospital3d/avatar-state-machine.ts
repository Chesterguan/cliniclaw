// Avatar command types for the hospital simulation visualization

import type { AvatarState } from './constants';

export interface AvatarCommand {
  agentName: string;
  encounterId: string;
  state: AvatarState;
  confidence?: number;
  error?: string;
  timestamp: number;
}

// Keep backward-compat alias
export type ConnectionCommand = AvatarCommand;
