export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="min-h-screen">
      <div className="border-b border-slate-700 bg-slate-800/50 px-6 py-3">
        <div className="flex items-center gap-4 max-w-5xl mx-auto">
          <h2 className="text-sm font-semibold text-slate-300">Admin</h2>
          <span className="text-xs text-slate-500">Informaticist Dashboard</span>
        </div>
      </div>
      {children}
    </div>
  );
}
