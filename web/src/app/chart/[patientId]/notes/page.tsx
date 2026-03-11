"use client";

/**
 * Ambient Documentation — three-state clinical note generation.
 *
 * State machine:
 *   input → processing → review → (sign or discard) → input
 *
 * This mirrors the Dragon Medical / AI scribe workflow:
 *   1. Clinician provides transcript (from recording or typed)
 *   2. System processes via Claude API (policy-gated)
 *   3. Clinician reviews the SOAP note before it writes to FHIR
 *
 * The "sign" action in a real system would create a FHIR DocumentReference.
 * Here we complete the loop by returning to the input state with confirmation.
 *
 * PHI note: The transcript textarea is NOT logged. The API call is gated by
 * policy (ambient_documentation capability). The audit event ID is shown so
 * the clinician can verify the trail.
 */

import { useState, useRef, useEffect } from "react";
import { useParams, useSearchParams } from "next/navigation";
import {
  Mic,
  FileText,
  CheckCircle,
  XCircle,
  Loader2,
  Clock,
  Pill,
  AlertCircle,
  ChevronDown,
  ChevronUp,
  Hash,
} from "lucide-react";
import { generateNote } from "@/lib/api";
import { usePatientContext } from "@/lib/patient-context";
import { PRACTITIONER_ID } from "@/lib/utils";
import { useConfidenceUI } from "@/hooks/use-confidence-ui";
import type { GenerateNoteResponse } from "@/lib/types";

type NoteState = "input" | "processing" | "review";

// SOAP sections returned in the report record
type SoapSection = "subjective" | "objective" | "assessment" | "plan";

const SOAP_LABELS: Record<SoapSection, string> = {
  subjective: "Subjective",
  objective: "Objective",
  assessment: "Assessment",
  plan: "Plan",
};

// Left-border accent color per SOAP section (clinical convention)
const SOAP_BORDER: Record<SoapSection, string> = {
  subjective: "border-l-4 border-l-blue-400",
  objective: "border-l-4 border-l-green-400",
  assessment: "border-l-4 border-l-amber-400",
  plan: "border-l-4 border-l-purple-400",
};

// Header text color per SOAP section
const SOAP_HEADER_COLOR: Record<SoapSection, string> = {
  subjective: "text-blue-700",
  objective: "text-green-700",
  assessment: "text-amber-700",
  plan: "text-purple-700",
};

// Section letter badge per SOAP section
const SOAP_LETTER: Record<SoapSection, string> = {
  subjective: "S",
  objective: "O",
  assessment: "A",
  plan: "P",
};

// Badge background per SOAP section
const SOAP_BADGE_BG: Record<SoapSection, string> = {
  subjective: "bg-blue-100 text-blue-700",
  objective: "bg-green-100 text-green-700",
  assessment: "bg-amber-100 text-amber-700",
  plan: "bg-purple-100 text-purple-700",
};

// Human-readable labels for confidence factors returned by the API
const CONFIDENCE_FACTOR_LABELS: Record<string, string> = {
  valid_json: "Valid JSON structure",
  complete_soap: "All SOAP sections present",
  has_icd10_codes: "ICD-10 codes extracted",
  has_assessment: "Assessment section present",
  has_plan: "Plan section present",
  has_subjective: "Subjective section present",
  has_objective: "Objective section present",
  note_length_ok: "Note length within range",
  no_hallucination_markers: "No hallucination markers detected",
  clinical_terminology: "Clinical terminology detected",
};

// ICD-10 code pattern — used to color-tag codes in the assessment section
const ICD10_RE = /\b[A-Z]\d{2}(?:\.\d{1,4})?\b/g;

function extractIcd10Codes(text: string): string[] {
  return [...new Set(text.match(ICD10_RE) ?? [])];
}

export default function NotesPage() {
  const params = useParams<{ patientId: string }>();
  const searchParams = useSearchParams();
  const encounterId = searchParams.get("encounter") ?? "";

  const { context } = usePatientContext();

  // Form state
  const [transcript, setTranscript] = useState("");
  const [chiefComplaint, setChiefComplaint] = useState("");

  // FSM state
  const [noteState, setNoteState] = useState<NoteState>("input");
  const [result, setResult] = useState<GenerateNoteResponse | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Elapsed timer for processing state
  const [elapsed, setElapsed] = useState(0);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Saved transcript for review display (frozen at submit time)
  const [submittedTranscript, setSubmittedTranscript] = useState("");

  // Signed confirmation
  const [signed, setSigned] = useState(false);

  // Start/stop elapsed timer
  useEffect(() => {
    if (noteState === "processing") {
      setElapsed(0);
      timerRef.current = setInterval(() => {
        setElapsed((e) => e + 1);
      }, 1000);
    } else {
      if (timerRef.current) {
        clearInterval(timerRef.current);
        timerRef.current = null;
      }
    }
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [noteState]);

  async function handleGenerate() {
    if (!transcript.trim()) return;

    setError(null);
    setSigned(false);
    setSubmittedTranscript(transcript);
    setNoteState("processing");

    try {
      const activeMedications = context?.activeMedications ?? [];
      const res = await generateNote(encounterId || params.patientId, {
        practitioner_id: PRACTITIONER_ID,
        transcript: transcript.trim(),
        chief_complaint: chiefComplaint.trim() || undefined,
        active_medications:
          activeMedications.length > 0 ? activeMedications : undefined,
        practitioner_role: "physician",
      });
      setResult(res);
      setNoteState("review");
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to generate note"
      );
      setNoteState("input");
    }
  }

  function handleSign() {
    // In production: POST to FHIR DocumentReference endpoint.
    // Here we simulate completion and return to input.
    setSigned(true);
    setTimeout(() => {
      setNoteState("input");
      setTranscript("");
      setChiefComplaint("");
      setResult(null);
      setSigned(false);
    }, 2000);
  }

  function handleDiscard() {
    setNoteState("input");
    setResult(null);
    setError(null);
  }

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="flex items-center justify-between mb-5">
        <div className="flex items-center gap-3">
          <Mic className="w-5 h-5 text-slate-600" />
          <h2 className="text-lg font-bold text-slate-900">
            Ambient Documentation
          </h2>
        </div>
        <StateIndicator state={noteState} />
      </div>

      {/* Signed success flash */}
      {signed && (
        <div className="mb-4 flex items-center gap-3 p-3 bg-green-50 border border-green-200 rounded-lg text-green-800">
          <CheckCircle className="w-5 h-5 text-green-600" />
          <div>
            <p className="font-semibold text-sm">Note signed and filed</p>
            <p className="text-xs mt-0.5 text-green-700">
              DocumentReference created · Returning to input…
            </p>
          </div>
        </div>
      )}

      {/* Error display */}
      {error && noteState === "input" && (
        <div className="mb-4 flex items-start gap-3 p-3 bg-red-50 border border-red-200 rounded-lg text-red-800">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5 text-red-600" />
          <div>
            <p className="font-semibold text-sm">Generation failed</p>
            <p className="text-xs mt-0.5 text-red-700">{error}</p>
          </div>
        </div>
      )}

      {/* ------------------------------------------------------------------ */}
      {/* STATE 1: Input                                                      */}
      {/* ------------------------------------------------------------------ */}
      {noteState === "input" && (
        <InputState
          transcript={transcript}
          setTranscript={setTranscript}
          chiefComplaint={chiefComplaint}
          setChiefComplaint={setChiefComplaint}
          activeMedications={context?.activeMedications ?? []}
          onGenerate={handleGenerate}
        />
      )}

      {/* ------------------------------------------------------------------ */}
      {/* STATE 2: Processing                                                 */}
      {/* ------------------------------------------------------------------ */}
      {noteState === "processing" && (
        <ProcessingState
          elapsed={elapsed}
          transcript={submittedTranscript}
        />
      )}

      {/* ------------------------------------------------------------------ */}
      {/* STATE 3: Review                                                     */}
      {/* ------------------------------------------------------------------ */}
      {noteState === "review" && result && (
        <ReviewState
          result={result}
          onSign={handleSign}
          onDiscard={handleDiscard}
        />
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// State indicator pill
// ---------------------------------------------------------------------------

function StateIndicator({ state }: { state: NoteState }) {
  const config = {
    input: {
      label: "Input",
      className: "bg-slate-100 text-slate-600 border border-slate-200",
    },
    processing: {
      label: "Processing",
      className:
        "bg-amber-50 text-amber-700 border border-amber-200 animate-clinical-pulse",
    },
    review: {
      label: "Review",
      className: "bg-blue-50 text-blue-700 border border-blue-200",
    },
  };
  const c = config[state];
  return (
    <span
      className={`px-3 py-1 text-xs font-semibold rounded-full ${c.className}`}
    >
      {c.label}
    </span>
  );
}

// ---------------------------------------------------------------------------
// Input State
// ---------------------------------------------------------------------------

function InputState({
  transcript,
  setTranscript,
  chiefComplaint,
  setChiefComplaint,
  activeMedications,
  onGenerate,
}: {
  transcript: string;
  setTranscript: (v: string) => void;
  chiefComplaint: string;
  setChiefComplaint: (v: string) => void;
  activeMedications: string[];
  onGenerate: () => void;
}) {
  const canGenerate = transcript.trim().length >= 20;

  return (
    <div className="space-y-5">
      {/* Chief complaint */}
      <div>
        <label className="block text-sm font-semibold text-slate-700 mb-1.5">
          Chief Complaint
          <span className="ml-1 text-slate-400 font-normal">(optional)</span>
        </label>
        <input
          type="text"
          value={chiefComplaint}
          onChange={(e) => setChiefComplaint(e.target.value)}
          placeholder="e.g., chest pain, shortness of breath, routine follow-up"
          className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm text-slate-900 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
      </div>

      {/* Transcript input */}
      <div>
        <label className="block text-sm font-semibold text-slate-700 mb-1.5">
          Clinical Transcript
          <span className="ml-1 text-slate-400 font-normal">
            (dictation or typed notes)
          </span>
        </label>
        <textarea
          value={transcript}
          onChange={(e) => setTranscript(e.target.value)}
          rows={12}
          placeholder={`Paste or type the clinical transcript here.\n\nExample:\n"Patient is a 45-year-old male presenting with a 3-day history of productive cough, fever of 38.5°C, and right-sided pleuritic chest pain. No hemoptysis. Vitals: BP 128/82, HR 96, RR 20, SpO2 95% on room air. On exam, decreased breath sounds right base with dullness to percussion. CXR shows right lower lobe consolidation..."`}
          className="w-full px-3 py-2.5 border border-slate-300 rounded-lg text-sm text-slate-900 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent font-mono leading-relaxed resize-none"
        />
        <p className="text-xs text-slate-400 mt-1">
          {transcript.length} characters
          {!canGenerate && transcript.length > 0 && (
            <span className="ml-2 text-amber-600">
              Minimum 20 characters required
            </span>
          )}
        </p>
      </div>

      {/* Active medications (read-only, from patient context) */}
      {activeMedications.length > 0 && (
        <div className="p-3 bg-slate-50 border border-slate-200 rounded-lg">
          <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide mb-2 flex items-center gap-1.5">
            <Pill className="w-3.5 h-3.5" />
            Active Medications (will be included in context)
          </p>
          <div className="flex flex-wrap gap-1.5">
            {activeMedications.map((med) => (
              <span
                key={med}
                className="px-2 py-0.5 bg-blue-50 text-blue-700 border border-blue-200 text-xs rounded"
              >
                {med}
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Generate button */}
      <div className="flex justify-end pt-1">
        <button
          onClick={onGenerate}
          disabled={!canGenerate}
          className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white text-sm font-semibold rounded-lg hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          <FileText className="w-4 h-4" />
          Generate Note
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Processing State
// ---------------------------------------------------------------------------

function ProcessingState({
  elapsed,
  transcript,
}: {
  elapsed: number;
  transcript: string;
}) {
  return (
    <div className="space-y-5">
      {/* Animated processing indicator */}
      <div className="flex flex-col items-center py-10 bg-white border border-slate-200 rounded-xl">
        <div className="relative mb-5">
          <Loader2 className="w-10 h-10 text-blue-500 animate-spin" />
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="w-3 h-3 bg-blue-500 rounded-full animate-clinical-pulse" />
          </div>
        </div>
        <h3 className="text-lg font-bold text-slate-800">
          Generating SOAP Note
        </h3>
        <p className="text-slate-500 text-sm mt-1">
          Claude is analyzing the transcript through VERITAS policy gate…
        </p>
        <div className="flex items-center gap-2 mt-4 text-slate-400 text-sm">
          <Clock className="w-4 h-4" />
          <span>
            {Math.floor(elapsed / 60)}:{String(elapsed % 60).padStart(2, "0")}
          </span>
        </div>
      </div>

      {/* Transcript read-only preview */}
      <div>
        <p className="text-xs font-semibold text-slate-500 uppercase tracking-wide mb-2">
          Submitted Transcript
        </p>
        <div className="p-3 bg-slate-50 border border-slate-200 rounded-lg text-sm text-slate-600 font-mono leading-relaxed max-h-48 overflow-auto clinical-scroll whitespace-pre-wrap">
          {transcript}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Review State
// ---------------------------------------------------------------------------

function ReviewState({
  result,
  onSign,
  onDiscard,
}: {
  result: GenerateNoteResponse;
  onSign: () => void;
  onDiscard: () => void;
}) {
  const report = result.report as Record<string, string>;
  const [expandedMeta, setExpandedMeta] = useState(false);
  const cui = useConfidenceUI(result.confidence);

  // Extract ICD-10 codes from assessment section
  const assessmentText = report.assessment ?? "";
  const icd10Codes = extractIcd10Codes(assessmentText);

  const soapSections: SoapSection[] = [
    "subjective",
    "objective",
    "assessment",
    "plan",
  ];

  // Confidence-driven review header styling
  const headerBg = cui.tier === 'high'
    ? 'bg-green-50 border-green-200'
    : cui.tier === 'low'
    ? 'bg-amber-50 border-amber-200'
    : 'bg-blue-50 border-blue-200';
  const headerIcon = cui.tier === 'high'
    ? 'text-green-600'
    : cui.tier === 'low'
    ? 'text-amber-600'
    : 'text-blue-600';
  const headerText = cui.tier === 'high'
    ? 'text-green-800'
    : cui.tier === 'low'
    ? 'text-amber-800'
    : 'text-blue-800';

  return (
    <div className="space-y-5">
      {/* Review header with confidence badge */}
      <div className={`flex items-center justify-between p-3 border rounded-lg ${headerBg}`}>
        <div className="flex items-center gap-3">
          <FileText className={`w-5 h-5 ${headerIcon}`} />
          <div>
            <p className={`text-sm font-semibold ${headerText}`}>
              {cui.tier === 'high' ? 'AI Confident — Review and Sign' :
               cui.tier === 'low' ? 'Review Carefully — Low Confidence' :
               'Note ready for review'}
            </p>
            <p className={`text-xs mt-0.5 ${cui.tier === 'low' ? 'text-amber-600' : cui.tier === 'high' ? 'text-green-600' : 'text-blue-600'}`}>
              {cui.tier === 'low'
                ? 'The AI is less certain about this output. Please review all sections carefully.'
                : 'Review the generated SOAP note before signing. Signing creates a FHIR DocumentReference.'}
            </p>
          </div>
        </div>
        {result.confidence && (
          <span className={`px-2 py-1 text-xs font-semibold rounded-full ${cui.badgeClass}`}>
            {Math.round(result.confidence.score * 100)}% {cui.badgeText}
          </span>
        )}
      </div>

      {/* SOAP sections — each in its own card with section-specific left border */}
      <div className="space-y-3">
        {soapSections.map((section) => {
          const text = report[section];
          if (!text) return null;
          return (
            <div
              key={section}
              className={`bg-white border border-slate-200 rounded-xl overflow-hidden ${SOAP_BORDER[section]}`}
            >
              <div className="px-5 pt-4 pb-1 flex items-center gap-2">
                <span
                  className={`inline-flex items-center justify-center w-6 h-6 rounded-full text-xs font-bold ${SOAP_BADGE_BG[section]}`}
                >
                  {SOAP_LETTER[section]}
                </span>
                <h4
                  className={`text-sm font-bold uppercase tracking-wide ${SOAP_HEADER_COLOR[section]}`}
                >
                  {SOAP_LABELS[section]}
                </h4>
              </div>
              <p className="px-5 pb-4 pt-2 text-sm text-slate-800 leading-relaxed whitespace-pre-wrap">
                {text}
              </p>
              {/* ICD-10 codes appear inline under Assessment */}
              {section === "assessment" && icd10Codes.length > 0 && (
                <div className="px-5 pb-4 flex items-center gap-2 flex-wrap">
                  <span className="text-xs text-amber-600 font-semibold uppercase tracking-wide">
                    ICD-10:
                  </span>
                  {icd10Codes.map((code) => (
                    <span
                      key={code}
                      className="px-2 py-0.5 bg-amber-50 text-amber-800 border border-amber-200 text-xs font-clinical-mono rounded-full font-semibold"
                    >
                      {code}
                    </span>
                  ))}
                </div>
              )}
            </div>
          );
        })}

        {/* Fallback: if report doesn't have SOAP keys, show raw */}
        {!soapSections.some((s) => report[s]) && (
          <div className="bg-white border border-slate-200 rounded-xl p-5">
            <h4 className="text-xs font-bold text-slate-500 uppercase tracking-widest mb-2">
              Note Content
            </h4>
            <pre className="text-sm text-slate-800 leading-relaxed whitespace-pre-wrap font-mono">
              {JSON.stringify(result.report, null, 2)}
            </pre>
          </div>
        )}
      </div>

      {/* VERITAS provenance block */}
      <div className="border border-slate-200 rounded-lg overflow-hidden">
        <button
          onClick={() => setExpandedMeta(!expandedMeta)}
          className="w-full flex items-center justify-between px-4 py-2.5 bg-slate-50 text-xs font-semibold text-slate-600 hover:bg-slate-100 transition-colors"
        >
          <span className="flex items-center gap-2">
            <Hash className="w-3.5 h-3.5" />
            VERITAS Provenance
          </span>
          {expandedMeta ? (
            <ChevronUp className="w-3.5 h-3.5" />
          ) : (
            <ChevronDown className="w-3.5 h-3.5" />
          )}
        </button>
        {expandedMeta && (
          <div className="px-4 py-3 space-y-3 text-xs">
            <MetaRow label="Audit Event ID" value={result.audit_event_id} mono />
            <MetaRow
              label="Spec Hash"
              value={result.spec_hash ?? "—"}
              mono
            />
            <MetaRow label="Status" value={result.status} />
            <MetaRow label="Policy Gate" value="ambient_documentation" />
            {result.confidence?.factors && result.confidence.factors.length > 0 && (
              <div className="flex items-start gap-3">
                <span className="text-slate-400 w-32 flex-shrink-0">
                  Confidence Factors
                </span>
                <div className="flex flex-wrap gap-1.5">
                  {result.confidence.factors.map((factor) => (
                    <span
                      key={factor}
                      className="px-2 py-0.5 bg-green-50 text-green-700 border border-green-200 rounded text-xs"
                    >
                      {CONFIDENCE_FACTOR_LABELS[factor] ?? factor}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Sign / Discard actions */}
      <div className="flex items-center justify-between pt-2">
        <button
          onClick={onDiscard}
          className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-red-600 bg-red-50 border border-red-200 rounded-lg hover:bg-red-100 transition-colors"
        >
          <XCircle className="w-4 h-4" />
          Discard
        </button>
        <button
          onClick={onSign}
          className="flex items-center gap-2 px-5 py-2.5 bg-green-600 text-white text-sm font-semibold rounded-lg hover:bg-green-700 transition-colors"
        >
          <CheckCircle className="w-4 h-4" />
          Sign Note
        </button>
      </div>
    </div>
  );
}

function MetaRow({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex items-start gap-3">
      <span className="text-slate-400 w-32 flex-shrink-0">{label}</span>
      <span
        className={`text-slate-700 break-all ${mono ? "font-clinical-mono" : ""}`}
      >
        {value}
      </span>
    </div>
  );
}
