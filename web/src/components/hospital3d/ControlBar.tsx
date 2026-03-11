'use client';

import Link from 'next/link';
import type { CameraPreset } from './CameraRig';

export type ViewMode = 'hospital' | 'network';

export type SimSpeed = 'demo' | 'normal' | 'fast';

interface ControlBarProps {
  connected: boolean;
  simRunning: boolean;
  onStart: () => void;
  cameraPreset: CameraPreset;
  onCameraPreset: (preset: CameraPreset) => void;
  simSpeed: SimSpeed;
  onSimSpeed: (speed: SimSpeed) => void;
  viewMode: ViewMode;
  onViewMode: (mode: ViewMode) => void;
}

export function ControlBar({
  connected,
  simRunning,
  onStart,
  cameraPreset,
  onCameraPreset,
  simSpeed,
  onSimSpeed,
  viewMode,
  onViewMode,
}: ControlBarProps) {
  return (
    <div className="absolute top-0 left-0 right-0 z-10 flex items-center justify-between px-4 py-1.5 bg-black/40 border-b border-white/5">
      {/* Left: wordmark + SSE dot */}
      <div className="flex items-center gap-3">
        <span className="text-white/70 text-xs tracking-widest font-light">
          ClinicClaw
        </span>
        {/* SSE status — just a dot, no label */}
        <div
          className={`w-1 h-1 rounded-full flex-shrink-0 ${
            connected ? 'bg-blue-400' : 'bg-white/15'
          }`}
          title={connected ? 'SSE connected' : 'SSE disconnected'}
        />
      </div>

      {/* Center: view toggle + camera presets */}
      <div className="flex items-center gap-3">
        {/* View mode toggle */}
        <div className="flex items-center gap-px bg-white/5 rounded-full p-0.5">
          {([
            ['hospital', 'Floor'],
            ['network', 'Graph'],
          ] as const).map(([key, label]) => (
            <button
              key={key}
              onClick={() => onViewMode(key)}
              className={`px-2.5 py-0.5 rounded-full text-[10px] tracking-wide transition-all ${
                viewMode === key
                  ? 'bg-white/15 text-white/80'
                  : 'text-white/25 hover:text-white/50'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        {/* Camera presets — pill-shaped ghost buttons */}
        <div className="flex items-center gap-1">
          {(
            [
              ['overview', 'Orbit'],
              ['top-down', 'Top'],
              ['close-up', 'Close'],
            ] as const
          ).map(([key, label]) => (
            <button
              key={key}
              onClick={() => onCameraPreset(key)}
              className={`px-2.5 py-0.5 rounded-full text-[10px] tracking-wide transition-all border ${
                cameraPreset === key
                  ? 'border-blue-400/60 text-blue-300 bg-blue-500/10'
                  : 'border-white/10 text-white/30 hover:text-white/60 hover:border-white/20 bg-transparent'
              }`}
            >
              {label}
            </button>
          ))}
        </div>
      </div>

      {/* Right: speed + start */}
      <div className="flex items-center gap-2">
        {/* Speed — segmented control, minimal */}
        <div className="flex items-center gap-px bg-white/5 rounded-full p-0.5">
          {(['demo', 'normal', 'fast'] as const).map((speed) => (
            <button
              key={speed}
              onClick={() => onSimSpeed(speed)}
              disabled={simRunning}
              className={`px-2.5 py-0.5 rounded-full text-[10px] tracking-wide transition-all font-mono ${
                simSpeed === speed
                  ? 'bg-white/15 text-white/80'
                  : 'text-white/25 hover:text-white/50'
              } ${simRunning ? 'cursor-not-allowed' : ''}`}
            >
              {speed === 'demo' ? 'demo' : speed === 'normal' ? '1×' : '3×'}
            </button>
          ))}
        </div>

        {/* Start — ghost style, matches the rest of the bar */}
        <button
          onClick={onStart}
          disabled={simRunning || !connected}
          className={`px-3 py-0.5 rounded-full text-[10px] tracking-wide border transition-all font-mono ${
            simRunning || !connected
              ? 'border-white/8 text-white/20 cursor-not-allowed'
              : 'border-blue-400/50 text-blue-300/80 hover:border-blue-400/80 hover:text-blue-200 hover:bg-blue-500/10'
          }`}
        >
          {simRunning ? 'running' : 'run sim'}
        </button>

        {/* 2D View — understated, far right */}
        <Link
          href="/hospital"
          className="text-[10px] text-white/20 hover:text-white/45 tracking-wide transition-colors font-mono"
        >
          2d
        </Link>
      </div>
    </div>
  );
}
