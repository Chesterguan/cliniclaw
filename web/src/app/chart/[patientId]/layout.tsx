"use client";

/**
 * Chart Layout — persistent patient banner + tab navigation.
 *
 * This is the outer shell for every chart tab (Notes, Orders, Prior Auth, Audit).
 * It mirrors how Epic Hyperspace renders a patient chart:
 *
 *   [Patient Banner — always visible]
 *   [Tab Bar: Notes | Orders | Prior Auth | Audit]
 *   [Tab content — rendered by child routes]
 *
 * The patient context is loaded once when the chart is first opened.
 * Subsequent tab switches do NOT re-fetch from FHIR (short-circuit in context).
 *
 * PatientProvider lives in the root layout, so this component just calls
 * loadPatient() and reads from the shared context.
 */

import { useEffect, useState } from "react";
import { useParams, useSearchParams, usePathname } from "next/navigation";
import Link from "next/link";
import {
  AlertTriangle,
  Pill,
  ClipboardList,
  ShoppingCart,
  FileText,
  ShieldCheck,
  MessageSquare,
  User,
  Loader2,
  AlertCircle,
  ChevronLeft,
  Activity,
  PanelRightClose,
} from "lucide-react";
import { usePatientContext } from "@/lib/patient-context";
import { formatAge, formatGender, getPatientName } from "@/lib/utils";
import { useEventStream } from "@/hooks/use-event-stream";
import { ActivityStream } from "@/components/clinical/activity-stream";

export default function ChartLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const params = useParams<{ patientId: string }>();
  const searchParams = useSearchParams();
  const encounterId = searchParams.get("encounter") ?? "";
  const patientId = params.patientId;

  const { context, loading, error, loadPatient } = usePatientContext();
  const [sidebarOpen, setSidebarOpen] = useState(true);

  // SSE event stream for real-time agent activity
  const { events, connected, clearEvents } = useEventStream({
    encounterId: encounterId || null,
  });

  // Load patient on mount and whenever patientId/encounterId changes.
  // loadPatient() short-circuits if already loaded for this patient+encounter.
  useEffect(() => {
    if (patientId && encounterId) {
      loadPatient(patientId, encounterId);
    }
  }, [patientId, encounterId, loadPatient]);

  return (
    <div className="flex flex-col h-full">
      {/* Patient Banner */}
      <PatientBanner
        patientId={patientId}
        encounterId={encounterId}
        context={context}
        loading={loading}
        error={error}
      />

      {/* Tab navigation with sidebar toggle */}
      <div className="flex items-center bg-white border-b border-slate-200 flex-shrink-0">
        <ChartTabBar patientId={patientId} encounterId={encounterId} />
        <div className="ml-auto pr-3">
          <button
            onClick={() => setSidebarOpen(!sidebarOpen)}
            className="relative flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium text-slate-500 hover:text-slate-700 transition-colors rounded-md hover:bg-slate-100"
            title={sidebarOpen ? "Hide agent activity" : "Show agent activity"}
          >
            {sidebarOpen ? (
              <PanelRightClose className="w-3.5 h-3.5" />
            ) : (
              <Activity className="w-3.5 h-3.5" />
            )}
            <span className="hidden sm:inline">
              {sidebarOpen ? "Hide" : "Activity"}
            </span>
            {/* Live indicator */}
            {connected && (
              <span className={`w-1.5 h-1.5 rounded-full ${
                events.length > 0 && events[events.length - 1]?.event_type.kind !== 'agent_completed'
                  ? 'bg-blue-500 animate-pulse'
                  : 'bg-emerald-500'
              }`} />
            )}
          </button>
        </div>
      </div>

      {/* Content area with optional sidebar */}
      <div className="flex-1 flex overflow-hidden">
        <div className="flex-1 overflow-auto clinical-scroll bg-slate-50">
          {children}
        </div>
        {sidebarOpen && (
          <div className="w-80 flex-shrink-0">
            <ActivityStream
              events={events}
              connected={connected}
              onClear={clearEvents}
            />
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Patient Banner
// ---------------------------------------------------------------------------

function PatientBanner({
  patientId,
  encounterId,
  context,
  loading,
  error,
}: {
  patientId: string;
  encounterId: string;
  context: ReturnType<typeof usePatientContext>["context"];
  loading: boolean;
  error: string | null;
}) {
  if (loading) {
    return (
      <div className="bg-slate-900 text-white px-6 py-3 flex items-center gap-3">
        <Loader2 className="w-4 h-4 animate-spin text-slate-400" />
        <span className="text-slate-400 text-sm">Loading patient chart…</span>
      </div>
    );
  }

  if (error) {
    return (
      <div className="bg-red-900 text-white px-6 py-3 flex items-center gap-3">
        <AlertCircle className="w-4 h-4 text-red-300" />
        <span className="text-red-200 text-sm">
          Failed to load patient: {error}
        </span>
      </div>
    );
  }

  if (!context) {
    return (
      <div className="bg-slate-900 text-white px-6 py-3 flex items-center gap-3">
        <User className="w-4 h-4 text-slate-500" />
        <span className="text-slate-500 text-sm">No patient loaded</span>
      </div>
    );
  }

  const { patient, encounter, allergies, problemList, activeMedications, flags } =
    context;

  const name = getPatientName(patient.name);
  const age = patient.birthDate ? formatAge(patient.birthDate) : "?";
  const gender = patient.gender ? formatGender(patient.gender) : "?";
  const encounterClass = encounter.class?.code?.toUpperCase() ?? "—";
  const location =
    encounter.location?.[0]?.location?.display ?? null;

  return (
    <div
      className={`text-white px-6 py-2.5 border-b ${flags.deceased ? "bg-red-900 border-red-800" : "bg-slate-900 border-slate-800"}`}
    >
      {/* Back to worklist */}
      <div className="mb-1.5">
        <Link
          href="/"
          className="flex items-center gap-1 text-xs text-slate-400 hover:text-slate-200 transition-colors"
        >
          <ChevronLeft className="w-3 h-3" />
          Back to Worklist
        </Link>
      </div>

      <div className="flex flex-wrap items-start gap-x-6 gap-y-2">
        {/* Name + demographics block */}
        <div className="flex items-start gap-3">
          <div className="w-9 h-9 rounded-full bg-slate-700 flex items-center justify-center flex-shrink-0 text-slate-300 font-bold text-sm">
            {name.charAt(0).toUpperCase()}
          </div>
          <div>
            <div className="flex items-center gap-2 flex-wrap">
              <h2 className="font-bold text-base leading-tight">{name}</h2>
              {/* Deceased hard stop — red banner within an already-red banner */}
              {flags.deceased && (
                <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-red-500 text-white text-xs font-bold rounded animate-pulse">
                  <AlertTriangle className="w-3 h-3" />
                  DECEASED
                </span>
              )}
            </div>
            <div className="flex flex-wrap items-center gap-x-3 gap-y-0.5 mt-0.5 text-xs text-slate-300">
              <span>
                {age}yo · {gender}
              </span>
              {patient.birthDate && (
                <span>DOB: {patient.birthDate}</span>
              )}
              <span className="font-clinical-mono text-slate-400">
                MRN: {patient.id}
              </span>
            </div>
          </div>
        </div>

        {/* Encounter info */}
        <div className="flex flex-col justify-center text-xs text-slate-300">
          <span className="text-slate-400 text-xs uppercase tracking-wide">
            Encounter
          </span>
          <span className="font-clinical-mono text-slate-200">
            {encounterId}
          </span>
          <div className="flex items-center gap-2 mt-0.5">
            <span className="bg-slate-700 px-1.5 py-0.5 rounded text-xs text-slate-200">
              {encounterClass}
            </span>
            {location && (
              <span className="text-slate-400">{location}</span>
            )}
          </div>
        </div>

        {/* Allergies — red when present, green for NKDA */}
        <div className="flex flex-col justify-center">
          <span className="text-slate-400 text-xs uppercase tracking-wide mb-1">
            Allergies
          </span>
          <div className="flex flex-wrap gap-1">
            {allergies.length === 0 ? (
              <span className="inline-flex items-center px-2 py-0.5 bg-green-900 text-green-300 border border-green-700 text-xs font-semibold rounded">
                NKDA
              </span>
            ) : (
              allergies.slice(0, 4).map((a) => (
                <span
                  key={a}
                  className="inline-flex items-center gap-1 px-2 py-0.5 bg-red-800 text-red-200 border border-red-700 text-xs rounded"
                >
                  <Pill className="w-2.5 h-2.5" />
                  {a}
                </span>
              ))
            )}
            {allergies.length > 4 && (
              <span className="text-xs text-red-400">
                +{allergies.length - 4}
              </span>
            )}
          </div>
        </div>

        {/* Active problems */}
        {problemList.length > 0 && (
          <div className="flex flex-col justify-center">
            <span className="text-slate-400 text-xs uppercase tracking-wide mb-1">
              Problems ({problemList.length})
            </span>
            <div className="flex flex-wrap gap-1">
              {problemList.slice(0, 3).map((p) => (
                <span
                  key={p.code}
                  className="inline-flex items-center gap-1 px-2 py-0.5 bg-slate-700 text-slate-200 text-xs rounded"
                  title={p.display}
                >
                  <span className="font-clinical-mono text-slate-400">
                    {p.code}
                  </span>
                  <span className="hidden lg:inline truncate max-w-32">
                    {p.display}
                  </span>
                </span>
              ))}
              {problemList.length > 3 && (
                <span className="text-xs text-slate-400">
                  +{problemList.length - 3} more
                </span>
              )}
            </div>
          </div>
        )}

        {/* Active medications count */}
        {activeMedications.length > 0 && (
          <div className="flex flex-col justify-center text-xs text-slate-300">
            <span className="text-slate-400 text-xs uppercase tracking-wide">
              Active Meds
            </span>
            <span className="mt-1 flex items-center gap-1.5">
              <Pill className="w-3.5 h-3.5 text-blue-400" />
              <span className="font-semibold">{activeMedications.length}</span>
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Chart Tab Bar
// ---------------------------------------------------------------------------

type ChartTab = {
  label: string;
  href: (patientId: string, encounterId: string) => string;
  icon: React.ReactNode;
  pathSegment: string;
};

const CHART_TABS: ChartTab[] = [
  {
    label: "Notes",
    href: (pid, eid) => `/chart/${pid}/notes?encounter=${eid}`,
    icon: <FileText className="w-3.5 h-3.5" />,
    pathSegment: "notes",
  },
  {
    label: "Orders",
    href: (pid, eid) => `/chart/${pid}/orders?encounter=${eid}`,
    icon: <ShoppingCart className="w-3.5 h-3.5" />,
    pathSegment: "orders",
  },
  {
    label: "Prior Auth",
    href: (pid, eid) => `/chart/${pid}/prior-auth?encounter=${eid}`,
    icon: <ClipboardList className="w-3.5 h-3.5" />,
    pathSegment: "prior-auth",
  },
  {
    label: "Review",
    href: (pid, eid) => `/chart/${pid}/review?encounter=${eid}`,
    icon: <MessageSquare className="w-3.5 h-3.5" />,
    pathSegment: "review",
  },
  {
    label: "Audit",
    href: (pid, eid) => `/chart/${pid}/audit-trail?encounter=${eid}`,
    icon: <ShieldCheck className="w-3.5 h-3.5" />,
    pathSegment: "audit-trail",
  },
];

function ChartTabBar({
  patientId,
  encounterId,
}: {
  patientId: string;
  encounterId: string;
}) {
  const pathname = usePathname();

  return (
    <div className="flex-1">
      <nav className="flex">
        {CHART_TABS.map((tab) => {
          const isActive = pathname.includes(`/${tab.pathSegment}`);
          return (
            <Link
              key={tab.pathSegment}
              href={tab.href(patientId, encounterId)}
              className={`
                flex items-center gap-2 px-5 py-2.5 text-sm font-medium border-b-2 transition-colors
                ${
                  isActive
                    ? "border-blue-500 text-blue-600 bg-blue-50"
                    : "border-transparent text-slate-600 hover:text-slate-900 hover:bg-slate-50"
                }
              `}
            >
              {tab.icon}
              {tab.label}
            </Link>
          );
        })}
      </nav>
    </div>
  );
}
