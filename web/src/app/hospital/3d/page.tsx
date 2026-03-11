'use client';

/**
 * 3D Hospital Simulation — cinema mode
 *
 * Full-screen canvas that covers the layout sidebar and header entirely
 * (fixed inset-0 z-50).  Agent avatars walk the hospital floor plan in
 * response to real-time SSE events from the simulation backend.
 *
 * No leva debug panel — lighting and scene values are baked into their
 * respective components.
 */

import { Suspense, useState, useCallback } from 'react';
import Link from 'next/link';
import { Canvas } from '@react-three/fiber';
import { useAllEventsStream } from '@/hooks/use-all-events-stream';
import { useSceneState } from '@/hooks/use-scene-state';
import { HospitalScene } from '@/components/hospital3d/HospitalScene';
import { NetworkScene } from '@/components/hospital3d/NetworkScene';
import { ControlBar, type ViewMode } from '@/components/hospital3d/ControlBar';
import { EventPanel } from '@/components/hospital3d/EventPanel';
import type { CameraPreset } from '@/components/hospital3d/CameraRig';

export default function Hospital3DPage() {
  const { events, connected, clearEvents } = useAllEventsStream({
    maxEvents: 2000,
    enabled: true,
  });

  const [simRunning, setSimRunning] = useState(false);
  const [cameraPreset, setCameraPreset] = useState<CameraPreset>('overview');
  const [simSpeed, setSimSpeed] = useState<'demo' | 'normal' | 'fast'>('demo');
  const [viewMode, setViewMode] = useState<ViewMode>('hospital');

  const handleStartSimulation = useCallback(async () => {
    clearEvents();
    useSceneState.getState().reset();
    setSimRunning(true);

    try {
      const res = await fetch('/api/v1/simulate', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ speed: simSpeed }),
      });

      if (!res.ok) {
        const body = await res.json().catch(() => ({ error: res.statusText }));
        console.error('Simulation error:', body.error || `HTTP ${res.status}`);
      }
    } catch (err) {
      console.error('Simulation failed:', err);
    } finally {
      // Longer timeout for demo speed so the running state persists through
      // the full agent lifecycle including linger + return walks.
      const timeout =
        simSpeed === 'demo' ? 120_000 : simSpeed === 'fast' ? 45_000 : 90_000;
      setTimeout(() => setSimRunning(false), timeout);
    }
  }, [clearEvents, simSpeed]);

  return (
    // fixed inset-0 covers layout sidebar and header — true cinema mode
    <div className="fixed inset-0 z-50 bg-black overflow-hidden">

      {/* 3D Canvas — fills the entire fixed container */}
      <Canvas
        shadows
        // Bird's-eye isometric-ish view centered on the hospital floor
        camera={{ position: [0, 30, 22], fov: 45, near: 0.1, far: 120 }}
        gl={{ antialias: true, alpha: false }}
        style={{ background: '#06060f' }}
      >
        <Suspense fallback={null}>
          {viewMode === 'hospital' ? (
            <HospitalScene events={events} cameraPreset={cameraPreset} />
          ) : (
            <NetworkScene events={events} cameraPreset={cameraPreset} />
          )}
        </Suspense>
      </Canvas>

      {/* Top control bar — camera presets, speed selector, run button */}
      <ControlBar
        connected={connected}
        simRunning={simRunning}
        onStart={handleStartSimulation}
        cameraPreset={cameraPreset}
        onCameraPreset={setCameraPreset}
        simSpeed={simSpeed}
        onSimSpeed={setSimSpeed}
        viewMode={viewMode}
        onViewMode={setViewMode}
      />

      {/* Bottom-right event log */}
      <EventPanel events={events} />

      {/* Back link — top-left corner, unobtrusive */}
      <Link
        href="/hospital"
        className="absolute top-9 left-4 z-10 text-[10px] font-mono text-white/20 hover:text-white/50 tracking-wide transition-colors"
        title="Exit cinema mode"
      >
        ← 2d
      </Link>

      {/* Idle hint — visible only before simulation starts */}
      {events.length === 0 && !simRunning && (
        <div className="absolute inset-0 flex items-center justify-center pointer-events-none z-0">
          <div className="text-center">
            <p className="text-slate-500 text-sm mb-1">
              Click{' '}
              <span className="text-blue-400 font-semibold">run sim</span>{' '}
              to begin
            </p>
            <p className="text-slate-600 text-xs">
              8 AI agents &middot; 6 patients &middot; real-time SSE
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
