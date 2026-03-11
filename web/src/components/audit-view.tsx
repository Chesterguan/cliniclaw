"use client";

/**
 * AuditView — shared audit trail component.
 *
 * Used by:
 *   - /audit (standalone, full chain verification)
 *   - /chart/[patientId]/audit-trail (in-chart, pre-filtered to patient)
 *
 * The VERITAS audit model requires every event to be linked in a hash chain.
 * This UI surfaces that chain so clinicians and compliance officers can verify
 * that no events have been tampered with. Colors:
 *   green = chain verified (integrity intact)
 *   red   = tamper detected (integrity violation — escalate immediately)
 *
 * Event outcome colors:
 *   success   → green
 *   denied    → red
 *   permitted → green
 *   error     → amber
 */

import { useState } from "react";
import useSWR from "swr";
import {
  ShieldCheck,
  ShieldAlert,
  Loader2,
  AlertCircle,
  Filter,
  RefreshCw,
  ChevronDown,
  ChevronUp,
  Hash,
  Clock,
  User,
  Activity,
} from "lucide-react";
import { fetchAuditEvents, verifyAuditChain } from "@/lib/api";
import { formatDateTime } from "@/lib/utils";
import type { AuditEvent, ChainVerification } from "@/lib/types";

interface AuditViewProps {
  /** Pre-fill the patient_id filter (used when rendered inside a chart). */
  initialPatientId?: string;
  /** Whether to show the chain verification panel. False in chart view. */
  showChainVerify?: boolean;
}

export function AuditView({
  initialPatientId = "",
  showChainVerify = true,
}: AuditViewProps) {
  const [patientFilter, setPatientFilter] = useState(initialPatientId);
  const [actionFilter, setActionFilter] = useState("");
  const [appliedPatient, setAppliedPatient] = useState(initialPatientId);
  const [appliedAction, setAppliedAction] = useState("");

  // Chain verification state (independent of event list)
  const [chainResult, setChainResult] = useState<ChainVerification | null>(null);
  const [verifying, setVerifying] = useState(false);
  const [verifyError, setVerifyError] = useState<string | null>(null);

  const {
    data: events,
    error: eventsError,
    isLoading,
    mutate,
  } = useSWR<AuditEvent[]>(
    ["audit-events", appliedPatient, appliedAction],
    () =>
      fetchAuditEvents({
        patient_id: appliedPatient || undefined,
        action: appliedAction || undefined,
      }),
    { refreshInterval: 30_000 }
  );

  function handleApplyFilters() {
    setAppliedPatient(patientFilter);
    setAppliedAction(actionFilter);
  }

  function handleClearFilters() {
    setPatientFilter(initialPatientId);
    setActionFilter("");
    setAppliedPatient(initialPatientId);
    setAppliedAction("");
  }

  async function handleVerifyChain() {
    setVerifying(true);
    setVerifyError(null);
    setChainResult(null);
    try {
      const result = await verifyAuditChain();
      setChainResult(result);
    } catch (err) {
      setVerifyError(
        err instanceof Error ? err.message : "Verification failed"
      );
    } finally {
      setVerifying(false);
    }
  }

  return (
    <div className="space-y-5">
      {/* Page title */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <ShieldCheck className="w-5 h-5 text-slate-600" />
          <h2 className="text-lg font-bold text-slate-900">Audit Trail</h2>
          {events && (
            <span className="text-sm text-slate-500">
              {events.length} event{events.length !== 1 ? "s" : ""}
            </span>
          )}
        </div>
        <button
          onClick={() => mutate()}
          aria-label="Refresh audit events"
          className="flex items-center gap-1.5 px-3 py-1.5 text-xs text-slate-600 bg-white border border-slate-200 rounded hover:bg-slate-50 transition-colors"
        >
          <RefreshCw className="w-3.5 h-3.5" />
          Refresh
        </button>
      </div>

      {/* Chain verification banner */}
      {showChainVerify && (
        <ChainVerificationBanner
          result={chainResult}
          verifying={verifying}
          error={verifyError}
          onVerify={handleVerifyChain}
        />
      )}

      {/* Filter controls */}
      <div className="bg-white border border-slate-200 rounded-xl p-4">
        <div className="flex items-center gap-2 mb-3">
          <Filter className="w-4 h-4 text-slate-400" />
          <span className="text-sm font-semibold text-slate-700">Filters</span>
        </div>
        <div className="flex flex-wrap gap-3">
          <div className="flex-1 min-w-48">
            <label className="block text-xs text-slate-500 mb-1">
              Patient ID
            </label>
            <input
              type="text"
              value={patientFilter}
              onChange={(e) => setPatientFilter(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleApplyFilters()}
              placeholder="patient-xxx"
              className="w-full px-3 py-1.5 border border-slate-300 rounded text-sm font-clinical-mono text-slate-900 placeholder-slate-400 placeholder:font-sans focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
          <div className="flex-1 min-w-48">
            <label className="block text-xs text-slate-500 mb-1">
              Action
            </label>
            <input
              type="text"
              value={actionFilter}
              onChange={(e) => setActionFilter(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleApplyFilters()}
              placeholder="e.g., ambient_documentation"
              className="w-full px-3 py-1.5 border border-slate-300 rounded text-sm text-slate-900 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            />
          </div>
          <div className="flex items-end gap-2">
            <button
              onClick={handleApplyFilters}
              className="px-4 py-1.5 bg-blue-600 text-white text-sm font-medium rounded hover:bg-blue-700 transition-colors"
            >
              Apply
            </button>
            <button
              onClick={handleClearFilters}
              className="px-3 py-1.5 text-sm text-slate-600 bg-slate-100 rounded hover:bg-slate-200 transition-colors"
            >
              Clear
            </button>
          </div>
        </div>
      </div>

      {/* Event list */}
      {isLoading && (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="w-6 h-6 text-blue-500 animate-spin" />
          <span className="ml-3 text-slate-500 text-sm">
            Loading audit events…
          </span>
        </div>
      )}

      {eventsError && (
        <div className="flex items-start gap-3 p-4 bg-red-50 border border-red-200 rounded-lg text-red-800">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
          <div>
            <p className="font-semibold text-sm">Failed to load audit events</p>
            <p className="text-xs mt-0.5 text-red-700">
              {eventsError instanceof Error
                ? eventsError.message
                : "Unknown error"}
            </p>
          </div>
        </div>
      )}

      {!isLoading && !eventsError && events?.length === 0 && (
        <div className="text-center py-12 text-slate-400 bg-white border border-dashed border-slate-200 rounded-xl">
          <Activity className="w-8 h-8 mx-auto mb-2 text-slate-300" />
          <p className="text-sm">No audit events found</p>
          {(appliedPatient || appliedAction) && (
            <p className="text-xs mt-1">
              Try clearing filters to see all events
            </p>
          )}
        </div>
      )}

      {events && events.length > 0 && (
        <div className="space-y-2">
          {events.map((event, idx) => (
            <AuditEventRow
              key={event.id}
              event={event}
              isFirst={idx === 0}
              isLast={idx === events.length - 1}
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Chain Verification Banner
// ---------------------------------------------------------------------------

function ChainVerificationBanner({
  result,
  verifying,
  error,
  onVerify,
}: {
  result: ChainVerification | null;
  verifying: boolean;
  error: string | null;
  onVerify: () => void;
}) {
  let bannerClass =
    "bg-white border border-slate-200 text-slate-700";
  let icon = <ShieldCheck className="w-5 h-5 text-slate-400" />;
  let message = "Chain integrity has not been verified yet.";

  if (result?.valid === true) {
    bannerClass = "bg-green-50 border border-green-300 text-green-800";
    icon = <ShieldCheck className="w-5 h-5 text-green-600" />;
    message = result.message;
  } else if (result?.valid === false) {
    bannerClass = "bg-red-50 border border-red-300 text-red-800";
    icon = <ShieldAlert className="w-5 h-5 text-red-600" />;
    message = result.message;
  } else if (error) {
    bannerClass = "bg-amber-50 border border-amber-200 text-amber-800";
    icon = <AlertCircle className="w-5 h-5 text-amber-600" />;
    message = error;
  }

  return (
    <div className={`flex items-center justify-between p-4 rounded-xl ${bannerClass}`}>
      <div className="flex items-start gap-3">
        {icon}
        <div>
          <p className="font-semibold text-sm">
            {result === null && !error
              ? "Hash Chain Integrity"
              : result?.valid
              ? "Chain Verified"
              : result?.valid === false
              ? "Tamper Detected — Escalate Immediately"
              : "Verification Error"}
          </p>
          <p className="text-xs mt-0.5 opacity-80">{message}</p>
        </div>
      </div>
      <button
        onClick={onVerify}
        disabled={verifying}
        aria-label="Verify audit chain integrity"
        className="flex items-center gap-2 px-4 py-2 bg-white border border-current border-opacity-30 text-sm font-medium rounded-lg hover:bg-opacity-80 disabled:opacity-50 transition-colors whitespace-nowrap ml-4"
      >
        {verifying ? (
          <>
            <Loader2 className="w-4 h-4 animate-spin" />
            Verifying…
          </>
        ) : (
          <>
            <ShieldCheck className="w-4 h-4" />
            Verify Chain
          </>
        )}
      </button>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Audit Event Row
// ---------------------------------------------------------------------------

function outcomeClass(outcome: string | undefined | null): string {
  if (!outcome) return "bg-slate-100 text-slate-700 border border-slate-200";
  switch (outcome.toLowerCase()) {
    case "success":
    case "permitted":
      return "bg-green-100 text-green-800 border border-green-200";
    case "denied":
    case "blocked":
    case "rejected":
      return "bg-red-100 text-red-800 border border-red-200";
    case "error":
    case "failed":
      return "bg-amber-100 text-amber-800 border border-amber-200";
    default:
      return "bg-slate-100 text-slate-700 border border-slate-200";
  }
}

function actionColor(action: string | undefined | null): string {
  if (!action) return "text-slate-600";
  if (action.includes("note") || action.includes("documentation")) {
    return "text-blue-600";
  }
  if (action.includes("order")) {
    return "text-purple-600";
  }
  if (action.includes("auth") || action.includes("prior")) {
    return "text-amber-600";
  }
  if (action.includes("audit") || action.includes("verify")) {
    return "text-green-600";
  }
  return "text-slate-600";
}

function AuditEventRow({
  event,
  isFirst,
  isLast,
}: {
  event: AuditEvent;
  isFirst: boolean;
  isLast: boolean;
}) {
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="bg-white border border-slate-200 rounded-xl overflow-hidden">
      {/* Main row */}
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full text-left px-4 py-3.5 flex items-start gap-4 hover:bg-slate-50 transition-colors"
      >
        {/* Timeline connector line (visual only) */}
        <div className="flex flex-col items-center flex-shrink-0 pt-1">
          <div className="w-2.5 h-2.5 rounded-full bg-blue-400 ring-2 ring-blue-100" />
          {!isLast && (
            <div className="w-px flex-1 bg-slate-200 mt-1 min-h-4" />
          )}
        </div>

        {/* Event content */}
        <div className="flex-1 min-w-0">
          <div className="flex items-start justify-between gap-2 flex-wrap">
            <div className="flex items-center gap-2 flex-wrap">
              <span
                className={`text-sm font-semibold ${actionColor(event.action)}`}
              >
                {event.action}
              </span>
              <span
                className={`px-2 py-0.5 text-xs font-semibold rounded-full ${outcomeClass(event.outcome)}`}
              >
                {event.outcome}
              </span>
            </div>
            <span className="text-xs text-slate-400 flex items-center gap-1 whitespace-nowrap">
              <Clock className="w-3 h-3" />
              {formatDateTime(event.timestamp)}
            </span>
          </div>

          <div className="flex flex-wrap gap-x-4 gap-y-0.5 mt-1.5 text-xs text-slate-500">
            <span className="flex items-center gap-1">
              <User className="w-3 h-3 text-slate-400" />
              <span className="font-clinical-mono">{event.actor_id}</span>
            </span>
            {event.patient_id && (
              <span className="flex items-center gap-1">
                <span className="text-slate-400">Patient:</span>
                <span className="font-clinical-mono">{event.patient_id}</span>
              </span>
            )}
            <span className="flex items-center gap-1">
              <Hash className="w-3 h-3 text-slate-400" />
              <span className="font-clinical-mono text-slate-400">
                {event.event_hash.slice(0, 16)}…
              </span>
            </span>
          </div>
        </div>

        {/* Expand toggle */}
        <div className="flex-shrink-0 text-slate-300">
          {expanded ? (
            <ChevronUp className="w-4 h-4" />
          ) : (
            <ChevronDown className="w-4 h-4" />
          )}
        </div>
      </button>

      {/* Expanded hash chain detail */}
      {expanded && (
        <div className="border-t border-slate-100 px-4 py-4 bg-slate-50 space-y-3">
          <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide">
            Hash Chain Detail
          </p>

          <div className="space-y-2">
            <HashRow label="Event ID" value={event.id} />
            <HashRow label="Event Hash" value={event.event_hash} />
            <HashRow label="Previous Hash" value={event.previous_hash} />
            <HashRow label="Input Hash" value={event.input_hash} />
            <HashRow label="Output Hash" value={event.output_hash} />
          </div>

          {/* Visual hash chain link */}
          <div className="flex items-center gap-2 text-xs text-slate-400 pt-1">
            <span className="font-clinical-mono truncate text-slate-500">
              {event.event_hash.slice(0, 20)}…
            </span>
            <span>←</span>
            <span className="font-clinical-mono truncate text-slate-400">
              {event.previous_hash.slice(0, 20)}…
            </span>
          </div>

          <p className="text-xs text-slate-400">
            Timestamp:{" "}
            <span className="font-clinical-mono text-slate-600">
              {event.timestamp}
            </span>
          </p>
        </div>
      )}
    </div>
  );
}

function HashRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start gap-3 text-xs">
      <span className="text-slate-400 w-28 flex-shrink-0">{label}</span>
      <span className="font-clinical-mono text-slate-700 break-all">
        {value}
      </span>
    </div>
  );
}
