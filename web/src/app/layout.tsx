import type { Metadata } from "next";
import "./globals.css";
import { PatientProvider } from "@/lib/patient-context";
import { NavLink } from "@/components/nav-link";
import { ErrorBoundary } from "@/components/error-boundary";
import { ClipboardList, ShieldCheck, LayoutDashboard, Hospital, Play } from "lucide-react";

export const metadata: Metadata = {
  title: "ClinicClaw — AI-Native HIS",
  description:
    "Clinical-grade Hospital Information System with VERITAS trust governance",
};

/**
 * Root layout — Apple-minimal × mission control.
 *
 * Structure: persistent left sidebar + thin top header + scrollable content area.
 * PatientProvider wraps everything so chart pages can access patient context
 * without prop-drilling across the route boundary.
 *
 * Design decisions:
 * - Sidebar is w-48 (slimmer than the old w-56) with near-black bg (#08090e).
 *   No right border — a box-shadow provides depth without a hard edge.
 * - Branding is text-only: "ClinicClaw" in Inter, with a 2px cyan accent line below.
 *   No icon — the name is the brand.
 * - Nav links use a left-border accent on hover instead of a filled background.
 *   Lighter weight, more breathing room between items.
 * - Sidebar footer: just "VERITAS" as a faint watermark. No Demo Mode badge — it's noise.
 * - Header is thinner (py-2), nearly transparent. Shows stack info on left,
 *   practitioner ID on right. No "ClinicClaw HIS" repetition — sidebar has branding.
 */
export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en">
      <body className="h-screen flex overflow-hidden" style={{ backgroundColor: "#f9fafb" }}>
        <PatientProvider>

          {/* ── Sidebar ──────────────────────────────────────────────────── */}
          {/*
           * Near-black background with a subtle right shadow instead of a hard border.
           * w-48 (192px) — slimmer than before, reducing chrome surface area.
           */}
          <aside
            className="w-48 flex-shrink-0 flex flex-col"
            style={{
              backgroundColor: "#08090e",
              boxShadow: "1px 0 0 0 #13151f, 4px 0 16px 0 rgba(0,0,0,0.35)",
            }}
          >
            {/* Branding block */}
            <div className="px-5 pt-6 pb-4">
              {/* Name only — Inter semibold, white. No icon. */}
              <span
                className="block text-white text-sm tracking-tight"
                style={{ fontWeight: 600, letterSpacing: "-0.01em" }}
              >
                ClinicClaw
              </span>
              {/* Thin cyan accent line — 2px, 24px wide, below the name */}
              <span
                className="block mt-2"
                style={{
                  height: "2px",
                  width: "24px",
                  backgroundColor: "#22d3ee",   /* cyan-400 */
                  borderRadius: "1px",
                }}
              />
            </div>

            {/* Navigation links */}
            {/*
             * More vertical spacing between items than before.
             * Hover: left 2px cyan border + subtle text brightening.
             * No filled background on hover — Chrome fades, content stays.
             */}
            <nav className="flex-1 px-3 py-2 flex flex-col gap-0.5">
              <NavLink href="/" icon={<ClipboardList className="w-3.5 h-3.5" />}>
                Worklist
              </NavLink>
              <NavLink href="/audit" icon={<ShieldCheck className="w-3.5 h-3.5" />}>
                Audit Trail
              </NavLink>
              <NavLink href="/admin" icon={<LayoutDashboard className="w-3.5 h-3.5" />}>
                Admin
              </NavLink>
              <NavLink href="/hospital" icon={<Hospital className="w-3.5 h-3.5" />}>
                Simulation
              </NavLink>
              <NavLink href="/demo" icon={<Play className="w-3.5 h-3.5" />}>
                Demo
              </NavLink>
            </nav>

            {/* Footer — VERITAS watermark only */}
            {/*
             * "VERITAS" in near-invisible monospace — a faint system watermark,
             * not a badge. No Demo Mode label — it adds noise without information.
             */}
            <div className="px-5 py-5">
              <span
                className="font-mono-data block"
                style={{
                  fontSize: "0.6rem",
                  letterSpacing: "0.18em",
                  color: "#2d3f55",             /* slightly brighter — ~12% visible against near-black bg */
                  textTransform: "uppercase",
                  userSelect: "none",
                }}
              >
                VERITAS
              </span>
              {/* Version + protocol tag — secondary footer line */}
              <span
                className="font-mono-data block mt-1.5"
                style={{
                  fontSize: "0.55rem",
                  letterSpacing: "0.12em",
                  color: "#1e293b",             /* dimmer than VERITAS label */
                  textTransform: "uppercase",
                  userSelect: "none",
                }}
              >
                v0.1.0 · FHIR R4
              </span>
            </div>
          </aside>

          {/* ── Main content area ─────────────────────────────────────────── */}
          <div className="flex-1 flex flex-col overflow-hidden">

            {/* Thin header — system info only, no branding repetition */}
            {/*
             * Nearly transparent: very subtle bottom border, no white box.
             * Left side: FHIR stack identifiers (these are static, informational).
             * Right side: practitioner ID in monospace.
             * No DEMO badge — already implied by the data context.
             */}
            <header
              className="flex-shrink-0 px-6 py-2 flex items-center justify-between"
              style={{
                borderBottom: "1px solid rgba(15,23,42,0.08)",  /* barely-there divider */
                backgroundColor: "rgba(249,250,251,0.85)",
                backdropFilter: "blur(8px)",
                WebkitBackdropFilter: "blur(8px)",
              }}
            >
              {/* Stack identifiers */}
              <div className="flex items-center gap-2">
                <span
                  className="font-mono-data"
                  style={{ color: "#94a3b8", fontSize: "0.6875rem" }}  /* slate-400, 11px */
                >
                  FHIR R4
                </span>
                <span style={{ color: "#cbd5e1", fontSize: "0.625rem" }}>·</span>
                <span
                  className="font-mono-data"
                  style={{ color: "#94a3b8", fontSize: "0.6875rem" }}
                >
                  Medplum
                </span>
                <span style={{ color: "#cbd5e1", fontSize: "0.625rem" }}>·</span>
                <span
                  className="font-mono-data"
                  style={{ color: "#94a3b8", fontSize: "0.6875rem" }}
                >
                  Claude
                </span>
              </div>

              {/* Practitioner identifier */}
              <div className="flex items-center gap-2">
                <span
                  className="font-mono-data"
                  style={{ color: "#94a3b8", fontSize: "0.6875rem" }}
                >
                  Practitioner
                </span>
                <span
                  className="font-mono-data"
                  style={{
                    color: "#475569",            /* slate-600 */
                    fontSize: "0.6875rem",
                  }}
                >
                  practitioner-001
                </span>
              </div>
            </header>

            {/* Page content */}
            <main className="flex-1 overflow-auto clinical-scroll">
              <ErrorBoundary>
                {children}
              </ErrorBoundary>
            </main>
          </div>

        </PatientProvider>
      </body>
    </html>
  );
}

