"use client";

/**
 * ClinicClaw Demo — Acute Chest Pain Encounter
 *
 * Scripted single-patient walkthrough for demo recording.
 * Shows the full VERITAS execution loop: triage → orders → lab → pharmacy →
 * documentation, with two human-in-the-loop approval gates.
 *
 * Last updated: 2026-03-09
 */

import { useEffect, useRef, useState, useCallback } from "react";
import {
  Activity,
  AlertCircle,
  AlertTriangle,
  CheckCircle2,
  ChevronRight,
  Clock,
  FileText,
  FlaskConical,
  Heart,
  Pill,
  RotateCcw,
  Shield,
  ShieldCheck,
  Stethoscope,
  Syringe,
  UserRound,
  Zap,
  ClipboardList,
  Database,
  Lock,
  BadgeCheck,
} from "lucide-react";

// ─── Types ────────────────────────────────────────────────────────────────────

type Phase =
  | "idle"
  | "triage_running"
  | "awaiting_order_approval"
  | "orders_running"
  | "lab_and_pharmacy_running"
  | "awaiting_med_approval"
  | "documentation_running"
  | "complete";

interface Patient {
  name: string;
  age: number;
  gender: string;
  chief_complaint: string;
  vitals: string;
  history: string[];
}

interface TriageResult {
  esi_level: number;
  acuity: string;
  recommendations: string[];
  confidence: number;
}

interface Order {
  order_type: string;
  description: string;
  fhir_resource_id: string;
}

interface LabResult {
  test: string;
  value: string;
  unit: string;
  interpretation: string;
  critical: boolean;
}

interface MedRecommendation {
  medication: string;
  dose: string;
  route: string;
  rationale: string;
  confidence: number;
}

interface SoapNote {
  subjective: string;
  objective: string;
  assessment: string;
  plan: string;
  icd10_codes: string[];
  confidence: number;
}

interface DemoStats {
  agents_run: number;
  human_approvals: number;
  policy_checks: number;
  fhir_writes: number;
  audit_events: number;
}

interface DemoState {
  phase: Phase;
  encounter_id: string;
  patient: Patient;
  triage_result: TriageResult | null;
  orders: Order[];
  lab_result: LabResult | null;
  med_recommendation: MedRecommendation | null;
  soap_note: SoapNote | null;
  stats: DemoStats;
}

type EventType =
  | "agent_start"
  | "agent_complete"
  | "policy_check"
  | "fhir_write"
  | "human_approval_requested"
  | "human_approval_granted"
  | "error";

interface AgentEvent {
  id: string;
  event_type: EventType;
  agent: string;
  encounter_id: string;
  message: string;
  timestamp: string;
}

// ─── Constants ────────────────────────────────────────────────────────────────

const PHASES: { key: Phase; label: string; icon: React.ReactNode }[] = [
  { key: "triage_running", label: "Triage", icon: <Stethoscope className="w-3 h-3" /> },
  { key: "awaiting_order_approval", label: "Orders", icon: <ClipboardList className="w-3 h-3" /> },
  { key: "lab_and_pharmacy_running", label: "Lab & Rx", icon: <FlaskConical className="w-3 h-3" /> },
  { key: "awaiting_med_approval", label: "Med Review", icon: <Pill className="w-3 h-3" /> },
  { key: "documentation_running", label: "SOAP Note", icon: <FileText className="w-3 h-3" /> },
  { key: "complete", label: "Complete", icon: <CheckCircle2 className="w-3 h-3" /> },
];

function phaseIndex(phase: Phase): number {
  if (phase === "idle") return -1;
  if (phase === "triage_running") return 0;
  if (phase === "awaiting_order_approval") return 1;
  if (phase === "orders_running") return 1;
  if (phase === "lab_and_pharmacy_running") return 2;
  if (phase === "awaiting_med_approval") return 3;
  if (phase === "documentation_running") return 4;
  if (phase === "complete") return 5;
  return -1;
}

const EVENT_STYLES: Record<
  EventType,
  { bg: string; text: string; border: string; label: string }
> = {
  agent_start: {
    bg: "rgba(59,130,246,0.10)",
    text: "#60a5fa",
    border: "rgba(59,130,246,0.20)",
    label: "AGENT START",
  },
  agent_complete: {
    bg: "rgba(16,185,129,0.10)",
    text: "#34d399",
    border: "rgba(16,185,129,0.20)",
    label: "AGENT DONE",
  },
  policy_check: {
    bg: "rgba(245,158,11,0.10)",
    text: "#fbbf24",
    border: "rgba(245,158,11,0.20)",
    label: "POLICY GATE",
  },
  fhir_write: {
    bg: "rgba(168,85,247,0.10)",
    text: "#c084fc",
    border: "rgba(168,85,247,0.20)",
    label: "FHIR WRITE",
  },
  human_approval_requested: {
    bg: "rgba(239,68,68,0.12)",
    text: "#f87171",
    border: "rgba(239,68,68,0.25)",
    label: "APPROVAL REQ",
  },
  human_approval_granted: {
    bg: "rgba(16,185,129,0.10)",
    text: "#34d399",
    border: "rgba(16,185,129,0.20)",
    label: "APPROVED",
  },
  error: {
    bg: "rgba(239,68,68,0.12)",
    text: "#f87171",
    border: "rgba(239,68,68,0.25)",
    label: "ERROR",
  },
};

// ESI → clinical color (matches ACS triage protocol)
function esiColor(level: number): string {
  if (level === 1) return "#ef4444"; // red — resuscitation
  if (level === 2) return "#f97316"; // orange — emergent
  if (level === 3) return "#f59e0b"; // amber — urgent
  if (level === 4) return "#22c55e"; // green — less urgent
  return "#64748b";                  // blue — non-urgent
}

function esiLabel(level: number): string {
  if (level === 1) return "Resuscitation";
  if (level === 2) return "Emergent";
  if (level === 3) return "Urgent";
  if (level === 4) return "Less Urgent";
  return "Non-urgent";
}

// Parse vitals string into individual values
function parseVitals(vitals: string): { label: string; value: string; alert?: boolean }[] {
  const items: { label: string; value: string; alert?: boolean }[] = [];
  const parts = vitals.split(",").map((s) => s.trim());
  for (const part of parts) {
    const [key, ...rest] = part.split(" ");
    const val = rest.join(" ");
    const label = key.replace(":", "");
    let alert = false;
    // Flag abnormal vitals
    if (label === "HR" && parseInt(val) > 100) alert = true;
    if (label === "BP") {
      const sys = parseInt(val.split("/")[0]);
      if (sys > 140 || sys < 90) alert = true;
    }
    if (label === "RR" && parseInt(val) > 20) alert = true;
    if (label === "SpO2") {
      const pct = parseInt(val.replace("%", ""));
      if (pct < 95) alert = true;
    }
    items.push({ label, value: val, alert });
  }
  return items;
}

// ─── API helpers ──────────────────────────────────────────────────────────────

async function fetchState(): Promise<DemoState> {
  const r = await fetch("/api/v1/demo/state");
  if (!r.ok) throw new Error(`state fetch failed: ${r.status}`);
  return r.json();
}

async function postAction(path: string): Promise<void> {
  const r = await fetch(path, { method: "POST" });
  if (!r.ok) throw new Error(`${path} failed: ${r.status}`);
}

// ─── Sub-components ───────────────────────────────────────────────────────────

function AnimatedCard({
  children,
  className = "",
  borderAccent,
}: {
  children: React.ReactNode;
  className?: string;
  borderAccent?: string;
}) {
  return (
    <div
      className={`animate-slide-up rounded-xl border ${className}`}
      style={{
        backgroundColor: "#0d1117",
        borderColor: borderAccent || "#1e293b",
        borderLeftWidth: borderAccent ? "3px" : undefined,
        borderLeftColor: borderAccent,
      }}
    >
      {children}
    </div>
  );
}

function CardHeading({
  icon,
  label,
  badge,
  color = "#22d3ee",
}: {
  icon: React.ReactNode;
  label: string;
  badge?: React.ReactNode;
  color?: string;
}) {
  return (
    <div className="flex items-center gap-2 px-4 pt-4 pb-2">
      <span style={{ color }}>{icon}</span>
      <span
        className="text-xs font-semibold uppercase tracking-widest"
        style={{ color: "#64748b", letterSpacing: "0.1em" }}
      >
        {label}
      </span>
      {badge && <span className="ml-auto">{badge}</span>}
    </div>
  );
}

function ConfidenceBadge({ score }: { score: number }) {
  const pct = Math.round(score * 100);
  const color = pct >= 90 ? "#34d399" : pct >= 70 ? "#fbbf24" : "#f87171";
  return (
    <span
      className="font-mono-data text-xs px-2 py-0.5 rounded flex items-center gap-1"
      style={{
        backgroundColor: `${color}15`,
        color,
        border: `1px solid ${color}30`,
      }}
    >
      <BadgeCheck className="w-3 h-3" />
      {pct}%
    </span>
  );
}

/** VERITAS policy badge — shows "VERITAS" trust mark on gated actions */
function VeritasBadge({ text = "VERITAS-GATED" }: { text?: string }) {
  return (
    <span
      className="font-mono-data text-xs px-2 py-0.5 rounded flex items-center gap-1"
      style={{
        backgroundColor: "rgba(245,158,11,0.08)",
        color: "#d97706",
        border: "1px solid rgba(245,158,11,0.2)",
        letterSpacing: "0.08em",
      }}
    >
      <Shield className="w-3 h-3" />
      {text}
    </span>
  );
}

// ── Patient Card ──────────────────────────────────────────────────────────────

function PatientCard({ patient }: { patient: Patient }) {
  const vitals = parseVitals(patient.vitals);
  return (
    <AnimatedCard>
      <div className="px-4 pt-4 pb-2 flex items-start justify-between">
        <div>
          <div className="flex items-baseline gap-3">
            <span className="text-white text-lg font-bold tracking-tight">
              {patient.name}
            </span>
            <span className="font-mono-data" style={{ color: "#64748b" }}>
              {patient.age}yo {patient.gender}
            </span>
          </div>
          <div className="flex items-center gap-3 mt-1">
            <span className="font-mono-data text-xs" style={{ color: "#475569" }}>
              MRN: 0042-7839-01
            </span>
            <span style={{ color: "#1e293b" }}>|</span>
            <span className="font-mono-data text-xs" style={{ color: "#475569" }}>
              DOB: 1963-11-14
            </span>
            <span style={{ color: "#1e293b" }}>|</span>
            <span className="font-mono-data text-xs" style={{ color: "#475569" }}>
              Allergies: NKDA
            </span>
          </div>
        </div>
        <span
          className="font-mono-data text-xs px-2 py-0.5 rounded"
          style={{
            backgroundColor: "rgba(239,68,68,0.08)",
            color: "#ef4444",
            border: "1px solid rgba(239,68,68,0.2)",
          }}
        >
          ED ENCOUNTER
        </span>
      </div>

      {/* Chief Complaint — prominent */}
      <div className="mx-4 mt-2 mb-3 px-3 py-2.5 rounded-lg" style={{
        backgroundColor: "rgba(239,68,68,0.06)",
        border: "1px solid rgba(239,68,68,0.15)",
      }}>
        <div className="flex items-center gap-2 mb-1.5">
          <AlertTriangle className="w-3.5 h-3.5" style={{ color: "#ef4444" }} />
          <span className="text-xs font-bold uppercase tracking-wider" style={{ color: "#ef4444" }}>
            Chief Complaint
          </span>
        </div>
        <p className="text-sm leading-relaxed" style={{ color: "#f1f5f9" }}>
          {patient.chief_complaint}
        </p>
      </div>

      {/* Vitals — individual badges */}
      <div className="px-4 mb-3">
        <p className="text-xs font-semibold uppercase tracking-wider mb-2" style={{ color: "#475569" }}>
          Vitals
        </p>
        <div className="flex flex-wrap gap-2">
          {vitals.map((v, i) => (
            <div
              key={i}
              className="flex items-baseline gap-1.5 px-2.5 py-1.5 rounded-lg"
              style={{
                backgroundColor: v.alert ? "rgba(239,68,68,0.06)" : "rgba(30,41,59,0.5)",
                border: `1px solid ${v.alert ? "rgba(239,68,68,0.2)" : "#1e293b"}`,
              }}
            >
              <span className="font-mono-data text-xs" style={{ color: "#64748b" }}>
                {v.label}
              </span>
              <span
                className="font-mono-data text-sm font-semibold"
                style={{ color: v.alert ? "#f87171" : "#e2e8f0" }}
              >
                {v.value}
              </span>
              {v.alert && <AlertTriangle className="w-3 h-3 flex-shrink-0" style={{ color: "#ef4444" }} />}
            </div>
          ))}
        </div>
      </div>

      {/* History */}
      {patient.history.length > 0 && (
        <div className="px-4 pb-4">
          <p className="text-xs font-semibold uppercase tracking-wider mb-2" style={{ color: "#475569" }}>
            Past Medical History
          </p>
          <div className="flex flex-wrap gap-1.5">
            {patient.history.map((h, i) => (
              <span
                key={i}
                className="text-xs px-2 py-1 rounded"
                style={{ backgroundColor: "rgba(30,41,59,0.6)", color: "#94a3b8", border: "1px solid #1e293b" }}
              >
                {h}
              </span>
            ))}
          </div>
        </div>
      )}
    </AnimatedCard>
  );
}

// ── Triage Card ───────────────────────────────────────────────────────────────

function TriageCard({ result }: { result: TriageResult }) {
  const col = esiColor(result.esi_level);
  return (
    <AnimatedCard borderAccent={col}>
      <CardHeading
        icon={<Stethoscope className="w-3.5 h-3.5" />}
        label="Triage Assessment"
        badge={<ConfidenceBadge score={result.confidence} />}
        color={col}
      />
      <div className="px-4 pb-4">
        <div className="flex items-center gap-4 mb-4">
          <div
            className="flex flex-col items-center justify-center w-16 h-16 rounded-xl"
            style={{
              backgroundColor: `${col}12`,
              border: `2px solid ${col}40`,
            }}
          >
            <span className="text-2xl font-black" style={{ color: col, lineHeight: 1 }}>
              {result.esi_level}
            </span>
            <span className="text-xs font-mono mt-0.5" style={{ color: `${col}90` }}>
              ESI
            </span>
          </div>
          <div>
            <p className="text-sm font-bold" style={{ color: col }}>
              {result.acuity || esiLabel(result.esi_level)}
            </p>
            <p className="text-xs mt-0.5" style={{ color: "#475569" }}>
              Emergency Severity Index · ACS Protocol
            </p>
          </div>
          <VeritasBadge text="POLICY: ALLOW" />
        </div>

        <p className="text-xs font-semibold uppercase tracking-wider mb-2" style={{ color: "#64748b" }}>
          Clinical Recommendations
        </p>
        <ul className="space-y-1.5">
          {result.recommendations.map((rec, i) => (
            <li key={i} className="flex items-start gap-2 text-sm" style={{ color: "#cbd5e1" }}>
              <ChevronRight className="w-3.5 h-3.5 flex-shrink-0 mt-0.5" style={{ color: col }} />
              {rec}
            </li>
          ))}
        </ul>
      </div>
    </AnimatedCard>
  );
}

// ── Orders Card ───────────────────────────────────────────────────────────────

function OrdersCard({ orders }: { orders: Order[] }) {
  return (
    <AnimatedCard borderAccent="#a78bfa">
      <CardHeading icon={<ClipboardList className="w-3.5 h-3.5" />} label="Orders Placed" color="#a78bfa" />
      <div className="px-4 pb-4">
        <ul className="space-y-2">
          {orders.map((order, i) => (
            <li
              key={i}
              className="flex items-center justify-between px-3 py-2.5 rounded-lg"
              style={{ backgroundColor: "rgba(168,85,247,0.05)", border: "1px solid rgba(168,85,247,0.12)" }}
            >
              <div className="flex items-center gap-2.5">
                <CheckCircle2 className="w-3.5 h-3.5 flex-shrink-0" style={{ color: "#a78bfa" }} />
                <span className="text-sm" style={{ color: "#e2e8f0" }}>{order.description}</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="font-mono-data text-xs" style={{ color: "#475569" }}>{order.order_type}</span>
                <Database className="w-3 h-3" style={{ color: "#475569" }} />
              </div>
            </li>
          ))}
        </ul>
        <div className="mt-3 flex items-center gap-2">
          <Lock className="w-3 h-3" style={{ color: "#475569" }} />
          <span className="font-mono-data text-xs" style={{ color: "#475569" }}>
            FHIR R4 ServiceRequest resources created · Audit trail recorded
          </span>
        </div>
      </div>
    </AnimatedCard>
  );
}

// ── Lab Result Card ───────────────────────────────────────────────────────────

function LabResultCard({ result }: { result: LabResult }) {
  const crit = result.critical;
  return (
    <AnimatedCard borderAccent={crit ? "#ef4444" : "#34d399"} className={crit ? "ring-1 ring-red-500/20" : ""}>
      <CardHeading
        icon={<FlaskConical className="w-3.5 h-3.5" />}
        label="Laboratory Result"
        color={crit ? "#ef4444" : "#34d399"}
        badge={
          crit ? (
            <span
              className="font-mono-data text-xs px-2 py-0.5 rounded flex items-center gap-1 animate-clinical-pulse"
              style={{
                backgroundColor: "rgba(239,68,68,0.12)",
                color: "#ef4444",
                border: "1px solid rgba(239,68,68,0.25)",
              }}
            >
              <AlertTriangle className="w-3 h-3" />
              CRITICAL VALUE
            </span>
          ) : null
        }
      />
      <div className="px-4 pb-4">
        <div
          className="flex items-center justify-between p-3 rounded-lg mb-3"
          style={{
            backgroundColor: crit ? "rgba(239,68,68,0.06)" : "rgba(16,185,129,0.06)",
            border: `1px solid ${crit ? "rgba(239,68,68,0.15)" : "rgba(16,185,129,0.15)"}`,
          }}
        >
          <div>
            <span className="text-sm font-bold" style={{ color: "#e2e8f0" }}>{result.test}</span>
            <p className="text-xs mt-0.5" style={{ color: "#475569" }}>
              Reference: &lt; 0.04 ng/mL
            </p>
          </div>
          <div className="flex items-baseline gap-1.5">
            <span className="text-3xl font-black" style={{ color: crit ? "#ef4444" : "#34d399" }}>
              {result.value}
            </span>
            <span className="font-mono-data text-sm" style={{ color: "#64748b" }}>{result.unit}</span>
            {crit && <AlertTriangle className="w-4 h-4 ml-1" style={{ color: "#ef4444" }} />}
          </div>
        </div>
        <p className="text-sm leading-relaxed" style={{ color: "#94a3b8" }}>{result.interpretation}</p>
      </div>
    </AnimatedCard>
  );
}

// ── Med Recommendation Card ───────────────────────────────────────────────────

function MedRecommendationCard({ rec }: { rec: MedRecommendation }) {
  return (
    <AnimatedCard borderAccent="#22d3ee">
      <CardHeading
        icon={<Pill className="w-3.5 h-3.5" />}
        label="Medication Recommendation"
        badge={<ConfidenceBadge score={rec.confidence} />}
        color="#22d3ee"
      />
      <div className="px-4 pb-4">
        <div
          className="flex items-center gap-4 p-3 rounded-lg mb-3"
          style={{ backgroundColor: "rgba(34,211,238,0.05)", border: "1px solid rgba(34,211,238,0.12)" }}
        >
          <Syringe className="w-5 h-5 flex-shrink-0" style={{ color: "#22d3ee" }} />
          <div className="flex-1">
            <span className="text-base font-bold" style={{ color: "#e2e8f0" }}>
              {rec.medication}
            </span>
            <div className="flex items-center gap-2 mt-0.5">
              <span className="font-mono-data text-sm font-semibold" style={{ color: "#22d3ee" }}>
                {rec.dose}
              </span>
              <span style={{ color: "#334155" }}>·</span>
              <span className="font-mono-data text-sm" style={{ color: "#94a3b8" }}>
                {rec.route}
              </span>
            </div>
          </div>
          <VeritasBadge text="REQUIRE_APPROVAL" />
        </div>
        <p className="text-sm leading-relaxed" style={{ color: "#94a3b8" }}>{rec.rationale}</p>
      </div>
    </AnimatedCard>
  );
}

// ── SOAP Note Card ────────────────────────────────────────────────────────────

const SOAP_COLORS: Record<string, string> = {
  subjective: "#60a5fa",
  objective: "#34d399",
  assessment: "#f59e0b",
  plan: "#a78bfa",
};

const SOAP_LABELS: Record<string, string> = {
  subjective: "S — Subjective",
  objective: "O — Objective",
  assessment: "A — Assessment",
  plan: "P — Plan",
};

function SoapNoteCard({ note }: { note: SoapNote }) {
  return (
    <AnimatedCard borderAccent="#10b981" className="ring-1 ring-emerald-500/15">
      <CardHeading
        icon={<FileText className="w-3.5 h-3.5" />}
        label="Clinical Documentation — SOAP Note"
        badge={<ConfidenceBadge score={note.confidence} />}
        color="#10b981"
      />
      <div className="px-4 pb-4 space-y-3">
        {(["subjective", "objective", "assessment", "plan"] as const).map((key) => {
          const color = SOAP_COLORS[key];
          const text = note[key];
          // Plan field — render numbered items if multi-line
          const isPlan = key === "plan";
          const planItems = isPlan ? text.split("\n").filter((l) => l.trim()) : [];

          return (
            <div key={key} className="rounded-lg px-3 py-2.5" style={{
              backgroundColor: `${color}08`,
              borderLeft: `3px solid ${color}`,
            }}>
              <p className="text-xs font-bold uppercase tracking-wider mb-1.5" style={{ color }}>
                {SOAP_LABELS[key]}
              </p>
              {isPlan && planItems.length > 1 ? (
                <ol className="space-y-1">
                  {planItems.map((item, i) => (
                    <li key={i} className="text-sm leading-relaxed" style={{ color: "#cbd5e1" }}>
                      {item.replace(/^\d+\.\s*/, "")}
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="text-sm leading-relaxed" style={{ color: "#cbd5e1" }}>{text}</p>
              )}
            </div>
          );
        })}

        {/* ICD-10 Codes */}
        {note.icd10_codes.length > 0 && (
          <div className="pt-2">
            <p className="text-xs font-semibold uppercase tracking-wider mb-2" style={{ color: "#475569" }}>
              ICD-10-CM Codes
            </p>
            <div className="flex flex-wrap gap-1.5">
              {note.icd10_codes.map((code) => (
                <span
                  key={code}
                  className="font-mono-data text-xs px-2.5 py-1 rounded-full"
                  style={{
                    backgroundColor: "rgba(245,158,11,0.08)",
                    color: "#d97706",
                    border: "1px solid rgba(245,158,11,0.2)",
                  }}
                >
                  {code}
                </span>
              ))}
            </div>
          </div>
        )}

        <div className="flex items-center gap-2 pt-1">
          <Lock className="w-3 h-3" style={{ color: "#475569" }} />
          <span className="font-mono-data text-xs" style={{ color: "#475569" }}>
            Written to FHIR R4 DocumentReference · SHA-256 audit hash recorded
          </span>
        </div>
      </div>
    </AnimatedCard>
  );
}

// ── Approval Panel ────────────────────────────────────────────────────────────

function ApprovalPanel({
  phase,
  state,
  onApprove,
  onReject,
  loading,
}: {
  phase: Phase;
  state: DemoState;
  onApprove: () => void;
  onReject: () => void;
  loading: boolean;
}) {
  const isOrderApproval = phase === "awaiting_order_approval";
  const isMedApproval = phase === "awaiting_med_approval";

  if (!isOrderApproval && !isMedApproval) return null;

  return (
    <div
      className="rounded-xl border p-4 animate-slide-up"
      style={{
        backgroundColor: "rgba(245,158,11,0.04)",
        borderColor: "rgba(245,158,11,0.25)",
        boxShadow: "0 0 20px rgba(245,158,11,0.06)",
      }}
    >
      <div className="flex items-center gap-2 mb-3">
        <ShieldCheck className="w-4 h-4" style={{ color: "#f59e0b" }} />
        <span className="text-sm font-bold" style={{ color: "#f59e0b" }}>
          {isOrderApproval ? "Clinician Order Approval Required" : "Medication Approval Required"}
        </span>
        <span
          className="font-mono-data text-xs px-1.5 py-0.5 rounded animate-clinical-pulse"
          style={{
            backgroundColor: "rgba(245,158,11,0.12)",
            color: "#f59e0b",
            border: "1px solid rgba(245,158,11,0.25)",
          }}
        >
          VERITAS POLICY GATE
        </span>
      </div>

      <div className="mb-4">
        {isOrderApproval && state.triage_result && (
          <div>
            <p className="text-xs font-semibold uppercase tracking-wider mb-2" style={{ color: "#64748b" }}>
              Review Proposed Orders from Triage AI (ESI-{state.triage_result.esi_level})
            </p>
            <ul className="space-y-1.5">
              {state.triage_result.recommendations.map((rec, i) => (
                <li key={i} className="text-sm flex items-center gap-2" style={{ color: "#cbd5e1" }}>
                  <ChevronRight className="w-3 h-3 flex-shrink-0" style={{ color: "#f59e0b" }} />
                  {rec}
                </li>
              ))}
            </ul>
            <p className="text-xs mt-2" style={{ color: "#475569" }}>
              Approving will create FHIR R4 ServiceRequest resources for each order.
            </p>
          </div>
        )}
        {isMedApproval && state.med_recommendation && (
          <div>
            <p className="text-xs font-semibold uppercase tracking-wider mb-2" style={{ color: "#64748b" }}>
              Review Medication Order
            </p>
            <div className="flex items-center gap-3 mb-2">
              <Pill className="w-4 h-4" style={{ color: "#22d3ee" }} />
              <span className="text-base font-bold" style={{ color: "#e2e8f0" }}>
                {state.med_recommendation.medication} {state.med_recommendation.dose} {state.med_recommendation.route}
              </span>
            </div>
            <p className="text-xs" style={{ color: "#94a3b8" }}>{state.med_recommendation.rationale}</p>
            <p className="text-xs mt-2" style={{ color: "#475569" }}>
              Approving will create a FHIR R4 MedicationRequest resource.
            </p>
          </div>
        )}
      </div>

      <div className="flex items-center gap-3">
        <button
          onClick={onApprove}
          disabled={loading}
          className="flex-1 flex items-center justify-center gap-2 py-3 rounded-lg text-sm font-bold transition-all"
          style={{
            backgroundColor: "#10b981",
            color: "white",
            boxShadow: "0 0 20px rgba(16,185,129,0.4), 0 0 6px rgba(16,185,129,0.3)",
            opacity: loading ? 0.6 : 1,
            animation: loading ? "none" : "approval-glow 2s ease-in-out infinite",
          }}
        >
          <CheckCircle2 className="w-4 h-4" />
          {loading ? "Processing…" : isOrderApproval ? "Approve Orders" : "Approve Medication"}
        </button>

        <button
          onClick={onReject}
          disabled={loading}
          className="px-4 py-3 rounded-lg text-sm transition-all"
          style={{
            backgroundColor: "rgba(239,68,68,0.06)",
            color: "#f87171",
            border: "1px solid rgba(239,68,68,0.15)",
            opacity: loading ? 0.6 : 1,
          }}
        >
          Reject
        </button>
      </div>
    </div>
  );
}

// ── Event Row ─────────────────────────────────────────────────────────────────

function EventRow({ event }: { event: AgentEvent }) {
  const style = EVENT_STYLES[event.event_type] ?? EVENT_STYLES.error;
  const ts = event.timestamp
    ? new Date(event.timestamp).toLocaleTimeString("en-US", {
        hour12: false,
        hour: "2-digit",
        minute: "2-digit",
        second: "2-digit",
      })
    : "";

  return (
    <div
      className="animate-slide-in px-3 py-2 rounded-lg border"
      style={{ backgroundColor: style.bg, borderColor: style.border }}
    >
      <div className="flex items-center gap-2 mb-0.5">
        <span
          className="font-mono-data text-xs px-1.5 py-0.5 rounded"
          style={{
            backgroundColor: `${style.text}18`,
            color: style.text,
            letterSpacing: "0.06em",
          }}
        >
          {style.label}
        </span>
        <span className="font-mono-data text-xs" style={{ color: "#64748b" }}>
          {event.agent}
        </span>
        <span className="ml-auto font-mono-data text-xs" style={{ color: "#334155" }}>
          {ts}
        </span>
      </div>
      <p className="text-xs leading-snug" style={{ color: "#94a3b8" }}>{event.message}</p>
    </div>
  );
}

// ── Stats Bar ─────────────────────────────────────────────────────────────────

function StatsBar({ stats }: { stats: DemoStats }) {
  const items = [
    { label: "Agents", value: stats.agents_run, color: "#60a5fa", icon: <Activity className="w-3 h-3" /> },
    { label: "Human Approvals", value: stats.human_approvals, color: "#fbbf24", icon: <ShieldCheck className="w-3 h-3" /> },
    { label: "Policy Gates", value: stats.policy_checks, color: "#f59e0b", icon: <Shield className="w-3 h-3" /> },
    { label: "FHIR Writes", value: stats.fhir_writes, color: "#a78bfa", icon: <Database className="w-3 h-3" /> },
    { label: "Audit Events", value: stats.audit_events, color: "#94a3b8", icon: <Lock className="w-3 h-3" /> },
  ];
  return (
    <div className="flex items-center gap-4">
      {items.map((item) => (
        <div key={item.label} className="flex items-center gap-1.5">
          <span style={{ color: item.color }}>{item.icon}</span>
          <span className="text-sm font-bold" style={{ color: item.color }}>{item.value}</span>
          <span className="font-mono-data text-xs" style={{ color: "#475569" }}>{item.label}</span>
        </div>
      ))}
    </div>
  );
}

// ── Phase Stepper ─────────────────────────────────────────────────────────────

function PhaseStepper({ phase }: { phase: Phase }) {
  const current = phaseIndex(phase);
  return (
    <div className="flex items-center gap-1">
      {PHASES.map((p, i) => {
        const done = current > i;
        const active = current === i;
        return (
          <div key={p.key} className="flex items-center gap-1">
            <div className="flex flex-col items-center gap-0.5">
              <div
                className="w-6 h-6 rounded-full flex items-center justify-center transition-all"
                style={{
                  backgroundColor: done ? "#10b981" : active ? "#22d3ee" : "rgba(30,41,59,0.6)",
                  color: done || active ? "white" : "#334155",
                  boxShadow: active ? "0 0 10px rgba(34,211,238,0.4)" : "none",
                }}
              >
                {done ? <CheckCircle2 className="w-3.5 h-3.5" /> : p.icon}
              </div>
              <span
                className="font-mono-data whitespace-nowrap"
                style={{
                  color: done ? "#10b981" : active ? "#22d3ee" : "#334155",
                  fontSize: "0.6rem",
                  letterSpacing: "0.05em",
                }}
              >
                {p.label}
              </span>
            </div>
            {i < PHASES.length - 1 && (
              <div
                className="w-6 h-0.5 mb-3 rounded transition-all"
                style={{ backgroundColor: done ? "#10b981" : "#1e293b" }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export default function DemoPage() {
  const [state, setState] = useState<DemoState | null>(null);
  const [events, setEvents] = useState<AgentEvent[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [actionLoading, setActionLoading] = useState(false);
  const [elapsedSec, setElapsedSec] = useState(0);
  const [startedAt, setStartedAt] = useState<number | null>(null);

  const eventStreamRef = useRef<EventSource | null>(null);
  const eventListRef = useRef<HTMLDivElement>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const eventIdRef = useRef(0);

  const refreshState = useCallback(async () => {
    try {
      const s = await fetchState();
      setState(s);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load demo state");
    }
  }, []);

  // Initial load
  useEffect(() => { refreshState(); }, [refreshState]);

  // Poll every 500ms while running
  useEffect(() => {
    if (!state) return;
    const running = state.phase !== "idle" && state.phase !== "complete";
    if (running) {
      pollRef.current = setInterval(refreshState, 500);
    } else {
      if (pollRef.current) clearInterval(pollRef.current);
    }
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, [state?.phase, refreshState]);

  // Elapsed timer
  useEffect(() => {
    if (!state) return;
    const running = state.phase !== "idle" && state.phase !== "complete";
    if (running && !startedAt) setStartedAt(Date.now());
    if (running && startedAt) {
      timerRef.current = setInterval(() => {
        setElapsedSec(Math.floor((Date.now() - startedAt) / 1000));
      }, 100);
    } else {
      if (timerRef.current) clearInterval(timerRef.current);
    }
    return () => { if (timerRef.current) clearInterval(timerRef.current); };
  }, [state?.phase, startedAt]);

  // SSE event stream
  useEffect(() => {
    if (!state) return;
    const running = state.phase !== "idle";
    if (running && !eventStreamRef.current) {
      const es = new EventSource("/api/v1/events");
      es.onmessage = (ev) => {
        try {
          const parsed = JSON.parse(ev.data) as Omit<AgentEvent, "id">;
          setEvents((prev) => [...prev, { ...parsed, id: String(eventIdRef.current++) }]);
        } catch { /* ignore malformed */ }
      };
      eventStreamRef.current = es;
    }
    if (!running) {
      eventStreamRef.current?.close();
      eventStreamRef.current = null;
    }
  }, [state?.phase]);

  useEffect(() => { return () => { eventStreamRef.current?.close(); }; }, []);

  // Auto-scroll event list
  useEffect(() => {
    if (eventListRef.current) {
      eventListRef.current.scrollTop = eventListRef.current.scrollHeight;
    }
  }, [events]);

  // Actions
  async function handleStart() {
    setActionLoading(true);
    setEvents([]);
    setElapsedSec(0);
    setStartedAt(Date.now());
    try {
      await postAction("/api/v1/demo/start");
      await refreshState();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Start failed");
    } finally { setActionLoading(false); }
  }

  async function handleApprove() {
    setActionLoading(true);
    try {
      await postAction("/api/v1/demo/approve");
      await refreshState();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Approve failed");
    } finally { setActionLoading(false); }
  }

  async function handleReset() {
    setActionLoading(true);
    eventStreamRef.current?.close();
    eventStreamRef.current = null;
    setEvents([]);
    setElapsedSec(0);
    setStartedAt(null);
    try {
      await postAction("/api/v1/demo/reset");
      await refreshState();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Reset failed");
    } finally { setActionLoading(false); }
  }

  // Derived state
  const phase = state?.phase ?? "idle";
  const isRunning = phase !== "idle" && phase !== "complete";
  const needsApproval = phase === "awaiting_order_approval" || phase === "awaiting_med_approval";

  const showTriage = state?.triage_result != null;
  const showOrders = state?.orders && state.orders.length > 0;
  const showLab = state?.lab_result != null;
  const showMedRec = state?.med_recommendation != null;
  const showSoap = state?.soap_note != null;

  function fmtElapsed(sec: number): string {
    const m = Math.floor(sec / 60);
    const s = sec % 60;
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }

  function phaseLabel(p: Phase): string {
    switch (p) {
      case "idle": return "Ready";
      case "triage_running": return "Triage Assessment";
      case "awaiting_order_approval": return "Awaiting Clinician Approval";
      case "orders_running": return "Placing Orders";
      case "lab_and_pharmacy_running": return "Lab & Pharmacy Review";
      case "awaiting_med_approval": return "Awaiting Med Approval";
      case "documentation_running": return "Generating SOAP Note";
      case "complete": return "Encounter Complete";
    }
  }

  return (
    <>
      <style>{`
        @keyframes approval-glow {
          0%, 100% { box-shadow: 0 0 16px rgba(16,185,129,0.4), 0 0 4px rgba(16,185,129,0.25); }
          50% { box-shadow: 0 0 30px rgba(16,185,129,0.6), 0 0 10px rgba(16,185,129,0.4); }
        }
      `}</style>

      <div className="h-full flex flex-col" style={{ backgroundColor: "#08090e", color: "#e2e8f0" }}>

        {/* ── Top bar ──────────────────────────────────────────────────────── */}
        <div
          className="flex-shrink-0 px-5 py-2.5 flex items-center gap-5"
          style={{ borderBottom: "1px solid #13151f", backgroundColor: "#08090e" }}
        >
          <div className="flex items-center gap-3 min-w-0">
            <Heart className="w-4 h-4 flex-shrink-0" style={{ color: "#ef4444" }} />
            <span className="text-sm font-bold text-white tracking-tight">ClinicClaw</span>
            <span
              className="font-mono-data text-xs px-2 py-0.5 rounded"
              style={{ backgroundColor: "#13151f", color: isRunning ? "#22d3ee" : "#334155", border: "1px solid #1e293b" }}
            >
              {isRunning ? fmtElapsed(elapsedSec) : "--:--"}
            </span>
            <span
              className="font-mono-data text-xs px-2 py-0.5 rounded"
              style={{
                backgroundColor: needsApproval ? "rgba(245,158,11,0.08)" : phase === "complete" ? "rgba(16,185,129,0.08)" : "rgba(34,211,238,0.06)",
                color: needsApproval ? "#f59e0b" : phase === "complete" ? "#10b981" : "#22d3ee",
                border: `1px solid ${needsApproval ? "rgba(245,158,11,0.2)" : phase === "complete" ? "rgba(16,185,129,0.2)" : "rgba(34,211,238,0.12)"}`,
              }}
            >
              {phaseLabel(phase)}
            </span>
          </div>

          <div className="flex-1 flex justify-center">
            <PhaseStepper phase={phase} />
          </div>

          {state && <StatsBar stats={state.stats} />}

          <div className="flex items-center gap-2 flex-shrink-0">
            {phase === "idle" && (
              <button
                onClick={handleStart}
                disabled={actionLoading}
                className="flex items-center gap-1.5 px-4 py-1.5 rounded-lg text-sm font-bold transition-all"
                style={{ backgroundColor: "#22d3ee", color: "#08090e", opacity: actionLoading ? 0.6 : 1 }}
              >
                <Zap className="w-3.5 h-3.5" />
                {actionLoading ? "Starting…" : "Start"}
              </button>
            )}
            <button
              onClick={handleReset}
              disabled={actionLoading}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-all"
              style={{ backgroundColor: "#13151f", color: "#64748b", border: "1px solid #1e293b", opacity: actionLoading ? 0.6 : 1 }}
            >
              <RotateCcw className="w-3 h-3" />
              Reset
            </button>
          </div>
        </div>

        {/* Error banner */}
        {error && (
          <div
            className="flex-shrink-0 flex items-center gap-2 px-5 py-2 text-xs"
            style={{ backgroundColor: "rgba(239,68,68,0.08)", borderBottom: "1px solid rgba(239,68,68,0.15)", color: "#f87171" }}
          >
            <AlertCircle className="w-3.5 h-3.5 flex-shrink-0" />
            {error}
          </div>
        )}

        {/* ── Main dual-pane ────────────────────────────────────────────────── */}
        <div className="flex-1 flex overflow-hidden">

          {/* ── Left: Clinician View (60%) ──────────────────────────────────── */}
          <div className="w-3/5 flex flex-col overflow-hidden" style={{ borderRight: "1px solid #13151f" }}>
            <div className="flex-shrink-0 flex items-center gap-2 px-5 py-2" style={{ borderBottom: "1px solid #13151f" }}>
              <Stethoscope className="w-3.5 h-3.5" style={{ color: "#22d3ee" }} />
              <span className="font-mono-data text-xs uppercase tracking-widest" style={{ color: "#475569" }}>
                Clinical View
              </span>
              {state?.encounter_id && (
                <span className="font-mono-data text-xs ml-2" style={{ color: "#334155" }}>
                  ENC: {state.encounter_id}
                </span>
              )}
            </div>

            <div className="flex-1 overflow-y-auto clinical-scroll-dark px-5 py-4 space-y-4">

              {/* Idle intro */}
              {phase === "idle" && !state && (
                <div className="flex flex-col items-center justify-center h-full">
                  <Activity className="w-10 h-10 mb-4" style={{ color: "#1e293b" }} />
                  <p className="text-sm" style={{ color: "#475569" }}>Loading…</p>
                </div>
              )}

              {phase === "idle" && state && (
                <div className="flex flex-col items-center justify-center min-h-64 text-center py-12">
                  <div className="flex items-center gap-2 mb-4">
                    <Heart className="w-6 h-6" style={{ color: "#ef4444" }} />
                    <AlertTriangle className="w-6 h-6" style={{ color: "#f59e0b" }} />
                  </div>
                  <h2 className="text-lg font-bold mb-1 text-white">
                    Acute Chest Pain — ED Encounter
                  </h2>
                  <p className="text-xs font-mono-data mb-4" style={{ color: "#475569" }}>
                    VERITAS-governed clinical workflow demonstration
                  </p>
                  <div className="text-sm max-w-md text-left space-y-2 mb-6" style={{ color: "#64748b", lineHeight: "1.7" }}>
                    <p>
                      A <span style={{ color: "#94a3b8" }}>62-year-old male</span> presents to the Emergency Department with{" "}
                      <span style={{ color: "#f87171" }}>acute substernal chest pain radiating to the left arm</span>,
                      diaphoresis, and tachycardia. Multiple cardiac risk factors.
                    </p>
                    <p>
                      ClinicClaw will orchestrate <span style={{ color: "#94a3b8" }}>6 AI agents</span> through the{" "}
                      <span style={{ color: "#d97706" }}>VERITAS trust kernel</span> — each action policy-gated,
                      FHIR-persisted, and cryptographically audited. Two{" "}
                      <span style={{ color: "#f59e0b" }}>human-in-the-loop approval gates</span> enforce
                      clinician oversight for orders and medications.
                    </p>
                  </div>
                  <button
                    onClick={handleStart}
                    disabled={actionLoading}
                    className="flex items-center gap-2 px-6 py-2.5 rounded-lg text-sm font-bold transition-all"
                    style={{ backgroundColor: "#22d3ee", color: "#08090e" }}
                  >
                    <Zap className="w-4 h-4" />
                    {actionLoading ? "Starting…" : "Begin Encounter"}
                  </button>
                </div>
              )}

              {/* Patient card */}
              {state && phase !== "idle" && <PatientCard patient={state.patient} />}

              {/* Triage result */}
              {state?.triage_result && showTriage && <TriageCard result={state.triage_result} />}

              {/* Approval panel */}
              {state && needsApproval && (
                <ApprovalPanel
                  phase={phase}
                  state={state}
                  onApprove={handleApprove}
                  onReject={handleReset}
                  loading={actionLoading}
                />
              )}

              {/* Orders */}
              {state && showOrders && state.orders.length > 0 && <OrdersCard orders={state.orders} />}

              {/* Lab result */}
              {state?.lab_result && showLab && <LabResultCard result={state.lab_result} />}

              {/* Med recommendation */}
              {state?.med_recommendation && showMedRec && <MedRecommendationCard rec={state.med_recommendation} />}

              {/* SOAP Note */}
              {state?.soap_note && showSoap && <SoapNoteCard note={state.soap_note} />}

              {/* Complete banner */}
              {phase === "complete" && (
                <AnimatedCard borderAccent="#10b981" className="ring-1 ring-emerald-500/20">
                  <div className="p-5 text-center">
                    <CheckCircle2 className="w-8 h-8 mx-auto mb-3" style={{ color: "#10b981" }} />
                    <p className="text-sm font-bold" style={{ color: "#10b981" }}>Encounter Complete</p>
                    <p className="text-xs mt-1 mb-4" style={{ color: "#475569" }}>
                      All clinical data persisted to FHIR R4 · Cryptographic audit chain intact · {elapsedSec}s elapsed
                    </p>
                    {state && (
                      <div className="flex justify-center mb-4">
                        <StatsBar stats={state.stats} />
                      </div>
                    )}
                    <button
                      onClick={handleReset}
                      className="flex items-center gap-2 mx-auto px-4 py-1.5 rounded-lg text-xs transition-all"
                      style={{ backgroundColor: "#13151f", color: "#64748b", border: "1px solid #1e293b" }}
                    >
                      <RotateCcw className="w-3 h-3" />
                      Run Again
                    </button>
                  </div>
                </AnimatedCard>
              )}

              {/* Running indicator */}
              {isRunning && !needsApproval && (
                <div className="flex items-center gap-2 px-2 py-1">
                  <div className="w-1.5 h-1.5 rounded-full animate-clinical-pulse" style={{ backgroundColor: "#22d3ee" }} />
                  <span className="font-mono-data text-xs" style={{ color: "#334155" }}>
                    {phaseLabel(phase)}…
                  </span>
                </div>
              )}
            </div>
          </div>

          {/* ── Right: Agent Stream (40%) ───────────────────────────────────── */}
          <div className="w-2/5 flex flex-col overflow-hidden">
            <div className="flex-shrink-0 flex items-center justify-between px-4 py-2" style={{ borderBottom: "1px solid #13151f" }}>
              <div className="flex items-center gap-2">
                <Activity className="w-3.5 h-3.5" style={{ color: "#22d3ee" }} />
                <span className="font-mono-data text-xs uppercase tracking-widest" style={{ color: "#475569" }}>
                  VERITAS Event Stream
                </span>
              </div>
              <span className="font-mono-data text-xs" style={{ color: "#334155" }}>
                {events.length} event{events.length !== 1 ? "s" : ""}
              </span>
            </div>

            {/* Legend */}
            <div className="flex-shrink-0 flex items-center gap-2.5 px-4 py-1.5 flex-wrap" style={{ borderBottom: "1px solid #0d1117" }}>
              {(Object.entries(EVENT_STYLES) as [EventType, (typeof EVENT_STYLES)[EventType]][])
                .filter(([k]) => k !== "human_approval_granted")
                .map(([key, s]) => (
                  <span key={key} className="font-mono-data flex items-center gap-1" style={{ color: s.text, fontSize: "0.6rem" }}>
                    <span className="w-1.5 h-1.5 rounded-full inline-block" style={{ backgroundColor: s.text }} />
                    {s.label}
                  </span>
                ))}
            </div>

            {/* Event feed */}
            <div ref={eventListRef} className="flex-1 overflow-y-auto clinical-scroll-dark px-3 py-2 space-y-1.5">
              {events.length === 0 && (
                <div className="flex flex-col items-center justify-center h-full text-center">
                  <Activity className="w-8 h-8 mb-3" style={{ color: "#1e293b" }} />
                  <p className="font-mono-data text-xs" style={{ color: "#334155" }}>
                    {phase === "idle" ? "Start encounter to see agent events" : "Connecting to VERITAS event stream…"}
                  </p>
                </div>
              )}
              {events.map((ev) => <EventRow key={ev.id} event={ev} />)}
            </div>
          </div>
        </div>
      </div>
    </>
  );
}
