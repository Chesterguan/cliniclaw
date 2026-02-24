"use client";

/**
 * Clinician Worklist — the landing page for ClinicClaw.
 *
 * This mirrors the Epic Storyboard or Cerner Message Center: a clinician opens
 * the system and sees their assigned patients sorted by acuity/encounter class.
 * Clicking a row opens the patient's chart.
 *
 * Sort order: inpatient (IMP) first, then by encounter start time ascending
 * (longest-admitted = most overdue for attention).
 *
 * Safety badges follow clinical convention:
 *   - Deceased = red background badge (hard stop color)
 *   - Allergies = red pill badges (allergy list critical color)
 *   - "NKDA" (No Known Drug Allergies) = green when empty
 */

import { useRouter } from "next/navigation";
import useSWR from "swr";
import {
  AlertTriangle,
  Clock,
  MapPin,
  Pill,
  Activity,
  User,
  ChevronRight,
  AlertCircle,
} from "lucide-react";
import { fetchWorklist } from "@/lib/api";
import {
  formatAge,
  formatGender,
  elapsedSince,
  PRACTITIONER_ID,
} from "@/lib/utils";
import type { WorklistEntry } from "@/lib/types";

// SWR fetcher wraps the API call; refresh every 60s for a live worklist feel.
const REFRESH_INTERVAL = 60_000;

function classCodeLabel(code: string): string {
  switch (code.toUpperCase()) {
    case "IMP":
      return "Inpatient";
    case "AMB":
      return "Ambulatory";
    case "EMER":
      return "Emergency";
    case "SS":
      return "Same-day Surgery";
    case "VR":
      return "Virtual";
    default:
      return code;
  }
}

function classCodeBadgeClass(code: string): string {
  switch (code.toUpperCase()) {
    case "IMP":
      // Inpatient — amber; these patients have the most urgency
      return "bg-amber-100 text-amber-800 border border-amber-300";
    case "EMER":
      return "bg-red-100 text-red-800 border border-red-200";
    case "AMB":
      return "bg-blue-100 text-blue-800 border border-blue-200";
    default:
      return "bg-slate-100 text-slate-700 border border-slate-200";
  }
}

/** Sort inpatient encounters to the top, then by start time. */
function sortWorklist(entries: WorklistEntry[]): WorklistEntry[] {
  return [...entries].sort((a, b) => {
    const aIsInpatient = a.encounter.class_code.toUpperCase() === "IMP";
    const bIsInpatient = b.encounter.class_code.toUpperCase() === "IMP";
    if (aIsInpatient && !bIsInpatient) return -1;
    if (!aIsInpatient && bIsInpatient) return 1;

    // Within the same class, sort by start time ascending (longest first)
    const aTime = a.encounter.start_time
      ? new Date(a.encounter.start_time).getTime()
      : 0;
    const bTime = b.encounter.start_time
      ? new Date(b.encounter.start_time).getTime()
      : 0;
    return aTime - bTime;
  });
}

export default function WorklistPage() {
  const router = useRouter();
  const {
    data,
    error,
    isLoading,
    mutate,
  } = useSWR<WorklistEntry[]>(
    ["worklist", PRACTITIONER_ID],
    () => fetchWorklist(PRACTITIONER_ID),
    { refreshInterval: REFRESH_INTERVAL }
  );

  function handleOpenChart(entry: WorklistEntry) {
    router.push(
      `/chart/${entry.patient.id}?encounter=${entry.encounter.id}`
    );
  }

  const sorted = data ? sortWorklist(data) : [];

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Page header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-xl font-bold text-slate-900">My Worklist</h1>
          <p className="text-slate-500 text-sm mt-0.5">
            Practitioner{" "}
            <span className="font-clinical-mono">{PRACTITIONER_ID}</span>
            {data && (
              <span className="ml-2 text-slate-400">
                · {data.length} patient{data.length !== 1 ? "s" : ""}
              </span>
            )}
          </p>
        </div>
        <button
          onClick={() => mutate()}
          className="flex items-center gap-2 px-3 py-1.5 text-sm text-slate-600 bg-white border border-slate-200 rounded hover:bg-slate-50 transition-colors"
        >
          <Activity className="w-3.5 h-3.5" />
          Refresh
        </button>
      </div>

      {/* Loading state */}
      {isLoading && (
        <div className="space-y-3">
          {[1, 2, 3].map((i) => (
            <div
              key={i}
              className="h-24 bg-white border border-slate-200 rounded-lg animate-pulse"
            />
          ))}
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="flex items-start gap-3 p-4 bg-red-50 border border-red-200 rounded-lg text-red-800">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5" />
          <div>
            <p className="font-semibold text-sm">Failed to load worklist</p>
            <p className="text-sm mt-0.5 text-red-700">
              {error instanceof Error ? error.message : "Unknown error"}
            </p>
            <button
              onClick={() => mutate()}
              className="mt-2 text-xs underline hover:no-underline"
            >
              Try again
            </button>
          </div>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && !error && sorted.length === 0 && (
        <div className="text-center py-16 text-slate-500">
          <User className="w-10 h-10 mx-auto mb-3 text-slate-300" />
          <p className="font-medium">No patients in worklist</p>
          <p className="text-sm mt-1">
            Patients will appear here when assigned to{" "}
            <span className="font-clinical-mono">{PRACTITIONER_ID}</span>
          </p>
        </div>
      )}

      {/* Worklist table */}
      {sorted.length > 0 && (
        <div className="space-y-2">
          {/* Column headers — match the clinical worklist column conventions */}
          <div className="hidden md:grid grid-cols-[2fr_1fr_1.5fr_1fr_1fr_auto] gap-4 px-4 py-1.5 text-xs font-semibold text-slate-500 uppercase tracking-wide">
            <span>Patient</span>
            <span>Class</span>
            <span>Primary Dx</span>
            <span>Location</span>
            <span>Time in</span>
            <span className="sr-only">Open</span>
          </div>

          {sorted.map((entry) => (
            <WorklistRow
              key={`${entry.patient.id}-${entry.encounter.id}`}
              entry={entry}
              onOpen={() => handleOpenChart(entry)}
            />
          ))}
        </div>
      )}

      {/* Demo data disclaimer */}
      <div className="mt-8 p-3 bg-amber-50 border border-amber-200 rounded text-amber-800 text-xs">
        <strong>MOCK DATA</strong> — All patient information is synthetic.
        Generated via Synthea-compatible test fixtures. No real PHI.
      </div>
    </div>
  );
}

/**
 * WorklistRow — one patient row in the clinician worklist.
 *
 * Matches the Epic Storyboard row pattern: patient demographics left, clinical
 * status center, safety flags prominent, one-click chart open.
 */
function WorklistRow({
  entry,
  onOpen,
}: {
  entry: WorklistEntry;
  onOpen: () => void;
}) {
  const { patient, encounter, allergies, problem_list, active_medications_count, flags } =
    entry;

  const primaryDx = problem_list[0] ?? null;
  const age = patient.birth_date ? formatAge(patient.birth_date) : "?";
  const gender = patient.gender ? formatGender(patient.gender) : "?";

  return (
    <button
      onClick={onOpen}
      className="w-full text-left bg-white border border-slate-200 rounded-lg px-4 py-3 hover:border-blue-300 hover:shadow-sm transition-all group"
    >
      <div className="md:grid md:grid-cols-[2fr_1fr_1.5fr_1fr_1fr_auto] gap-4 items-center">
        {/* Patient demographics */}
        <div className="flex items-start gap-3">
          <div className="w-8 h-8 rounded-full bg-slate-100 flex items-center justify-center flex-shrink-0 text-slate-500 text-xs font-bold mt-0.5">
            {patient.name.charAt(0).toUpperCase()}
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="font-semibold text-slate-900 text-sm">
                {patient.name}
              </span>
              {/* Deceased flag — red hard stop badge */}
              {flags.deceased && (
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 bg-red-600 text-white text-xs font-bold rounded">
                  <AlertTriangle className="w-2.5 h-2.5" />
                  DECEASED
                </span>
              )}
            </div>
            <div className="text-slate-500 text-xs mt-0.5">
              <span className="font-clinical-mono">{patient.id}</span>
              <span className="mx-1">·</span>
              <span>{age}yo {gender}</span>
            </div>
            {/* Allergy pills — red per clinical color language */}
            <div className="flex flex-wrap gap-1 mt-1.5">
              {allergies.length === 0 ? (
                <span className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-green-50 text-green-700 border border-green-200 rounded-full">
                  NKDA
                </span>
              ) : (
                allergies.slice(0, 3).map((allergy) => (
                  <span
                    key={allergy}
                    className="inline-flex items-center gap-1 px-1.5 py-0.5 text-xs bg-red-50 text-red-700 border border-red-200 rounded-full"
                  >
                    <Pill className="w-2.5 h-2.5" />
                    {allergy}
                  </span>
                ))
              )}
              {allergies.length > 3 && (
                <span className="text-xs text-red-600">
                  +{allergies.length - 3} more
                </span>
              )}
            </div>
          </div>
        </div>

        {/* Encounter class badge */}
        <div>
          <span
            className={`inline-block px-2 py-0.5 text-xs font-semibold rounded ${classCodeBadgeClass(encounter.class_code)}`}
          >
            {classCodeLabel(encounter.class_code)}
          </span>
        </div>

        {/* Primary diagnosis */}
        <div className="mt-2 md:mt-0">
          {primaryDx ? (
            <div>
              <p className="text-sm text-slate-800 truncate" title={primaryDx.display}>
                {primaryDx.display}
              </p>
              <p className="text-xs text-slate-500 font-clinical-mono mt-0.5">
                {primaryDx.code}
              </p>
            </div>
          ) : (
            <span className="text-xs text-slate-400">No primary Dx</span>
          )}
          {/* Problem and medication counts */}
          <div className="flex gap-3 mt-1">
            {problem_list.length > 0 && (
              <span className="text-xs text-slate-500">
                {problem_list.length} problem{problem_list.length !== 1 ? "s" : ""}
              </span>
            )}
            {active_medications_count > 0 && (
              <span className="text-xs text-slate-500 flex items-center gap-0.5">
                <Pill className="w-3 h-3" />
                {active_medications_count} med{active_medications_count !== 1 ? "s" : ""}
              </span>
            )}
          </div>
        </div>

        {/* Location */}
        <div className="mt-1 md:mt-0">
          {encounter.location ? (
            <span className="flex items-center gap-1 text-sm text-slate-600">
              <MapPin className="w-3.5 h-3.5 text-slate-400" />
              <span className="truncate">{encounter.location}</span>
            </span>
          ) : (
            <span className="text-xs text-slate-400">—</span>
          )}
        </div>

        {/* Elapsed time */}
        <div className="mt-1 md:mt-0">
          {encounter.start_time ? (
            <span className="flex items-center gap-1 text-sm text-slate-600">
              <Clock className="w-3.5 h-3.5 text-slate-400" />
              {elapsedSince(encounter.start_time)}
            </span>
          ) : (
            <span className="text-xs text-slate-400">—</span>
          )}
        </div>

        {/* Open chevron */}
        <div className="hidden md:block text-slate-300 group-hover:text-blue-400 transition-colors">
          <ChevronRight className="w-5 h-5" />
        </div>
      </div>
    </button>
  );
}
