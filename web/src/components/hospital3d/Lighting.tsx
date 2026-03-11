'use client';

/**
 * Lighting — static scene lighting for the 3D hospital floor visualization.
 *
 * Values are baked in from the previous leva-tuned defaults so the scene looks
 * identical without the debug panel.
 */
export function Lighting() {
  return (
    <>
      {/* Soft fill from above */}
      <ambientLight intensity={0.25} color="#8090b0" />

      {/* Key light — primary shadow caster, top-right-front */}
      <directionalLight
        position={[10, 20, 5]}
        intensity={0.7}
        color="#e8e0f0"
        castShadow
        shadow-mapSize-width={2048}
        shadow-mapSize-height={2048}
        shadow-camera-left={-25}
        shadow-camera-right={25}
        shadow-camera-top={15}
        shadow-camera-bottom={-15}
        shadow-camera-far={50}
        shadow-bias={-0.001}
      />

      {/* Fill light — top-left-back, cool blue to balance warmth */}
      <directionalLight position={[-8, 12, -6]} intensity={0.25} color="#4488cc" />

      {/* Rim light — low back, subtle purple accent */}
      <directionalLight position={[0, -5, -15]} intensity={0.15} color="#6644aa" />

      {/* Hemisphere — sky/ground gradient */}
      <hemisphereLight color="#c0d0ff" groundColor="#0a0a1a" intensity={0.35} />

      {/* Floor-level dark fill — keeps the ground dark */}
      <pointLight position={[0, -1, 0]} intensity={0.6} color="#1a1a3a" distance={20} decay={2} />

      {/* Accent points — one per quadrant, colored by department side */}
      <pointLight position={[12, 8, 0]}   intensity={0.3} color="#3b82f6" distance={18} decay={2} />
      <pointLight position={[-12, 8, 0]}  intensity={0.3} color="#8b5cf6" distance={18} decay={2} />
      <pointLight position={[0, 8, 12]}   intensity={0.2} color="#06b6d4" distance={16} decay={2} />
      <pointLight position={[0, 8, -12]}  intensity={0.2} color="#ec4899" distance={16} decay={2} />
    </>
  );
}
