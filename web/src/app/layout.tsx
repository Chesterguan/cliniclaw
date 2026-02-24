import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";
import { PatientProvider } from "@/lib/patient-context";
import { ClipboardList, ShieldCheck, Activity, LayoutDashboard } from "lucide-react";

export const metadata: Metadata = {
  title: "ClinicClaw — AI-Native HIS",
  description:
    "Clinical-grade Hospital Information System with VERITAS trust governance",
};

/**
 * Root layout.
 *
 * Structure mirrors how Epic Hyperspace frames its workspace:
 * - Persistent left sidebar (navigation rail)
 * - Top header with system branding
 * - Content area fills remaining space
 *
 * PatientProvider wraps everything so chart pages can access patient context
 * without prop-drilling across the route boundary.
 */
export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="h-screen flex overflow-hidden bg-slate-50">
        <PatientProvider>
          {/* Sidebar nav — slate-900 background matches clinical dark chrome */}
          <aside className="w-56 flex-shrink-0 bg-slate-900 border-r border-slate-800 flex flex-col">
            {/* System branding */}
            <div className="px-4 py-4 border-b border-slate-800">
              <div className="flex items-center gap-2">
                <Activity className="w-5 h-5 text-blue-400" strokeWidth={2} />
                <span className="text-white font-bold text-sm tracking-wide">
                  ClinicClaw
                </span>
              </div>
              <p className="text-slate-400 text-xs mt-0.5">
                AI-Native HIS
              </p>
            </div>

            {/* Navigation links */}
            <nav className="flex-1 px-2 py-3 space-y-0.5">
              <NavLink href="/" icon={<ClipboardList className="w-4 h-4" />}>
                Worklist
              </NavLink>
              <NavLink href="/audit" icon={<ShieldCheck className="w-4 h-4" />}>
                Audit Trail
              </NavLink>
              <NavLink href="/admin" icon={<LayoutDashboard className="w-4 h-4" />}>
                Admin
              </NavLink>
            </nav>

            {/* Footer — demo mode indicator */}
            <div className="px-4 py-3 border-t border-slate-800">
              <div className="demo-badge w-full justify-center">
                Demo Mode
              </div>
              <p className="text-slate-500 text-xs mt-1.5 text-center">
                VERITAS Trust Layer
              </p>
            </div>
          </aside>

          {/* Main content area */}
          <div className="flex-1 flex flex-col overflow-hidden">
            {/* Top header bar */}
            <header className="bg-white border-b border-slate-200 px-6 py-2.5 flex items-center justify-between flex-shrink-0">
              <div className="flex items-center gap-3">
                <span className="text-slate-800 font-semibold text-sm">
                  ClinicClaw HIS
                </span>
                <span className="text-slate-300 text-xs">|</span>
                <span className="text-slate-500 text-xs">
                  FHIR R4 · Medplum · Claude API
                </span>
              </div>
              <div className="flex items-center gap-3 text-xs text-slate-500">
                <span>
                  Practitioner:{" "}
                  <span className="font-clinical-mono text-slate-700">
                    practitioner-001
                  </span>
                </span>
                <span className="demo-badge">DEMO</span>
              </div>
            </header>

            {/* Page content */}
            <main className="flex-1 overflow-auto clinical-scroll">
              {children}
            </main>
          </div>
        </PatientProvider>
      </body>
    </html>
  );
}

/**
 * NavLink — sidebar navigation item.
 *
 * We use an anchor element via Next's Link. Active state is handled via
 * CSS (aria-current would require a client component with usePathname).
 * For now, the hover state provides sufficient affordance.
 */
function NavLink({
  href,
  icon,
  children,
}: {
  href: string;
  icon: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <Link
      href={href}
      className="flex items-center gap-3 px-3 py-2 rounded text-slate-400 hover:text-white hover:bg-slate-800 transition-colors text-sm"
    >
      {icon}
      {children}
    </Link>
  );
}
