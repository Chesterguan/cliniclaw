"use client";

/**
 * Prior Authorization — assembles and submits PA packages.
 *
 * Clinical workflow:
 *   1. Clinician enters the service description, CPT codes, ICD-10 codes,
 *      and clinical justification notes
 *   2. "Assemble PA Package" → API call → Claude generates clinical
 *      justification and packages the request (VERITAS policy-gated)
 *   3. Result shows the assembled package for clinician review
 *   4. "Approve & Submit" finalizes the PA request (FHIR ServiceRequest +
 *      ClaimResponse pattern per CMS-0057)
 *
 * Status badge colors follow clinical convention:
 *   pending_approval → amber (requires physician review)
 *   approved         → green
 *   denied           → red
 *   submitted        → blue
 */

import { useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import {
  ClipboardList,
  Plus,
  X,
  Loader2,
  CheckCircle,
  XCircle,
  AlertCircle,
  FileText,
  ChevronDown,
  ChevronUp,
} from "lucide-react";
import { assemblePriorAuth } from "@/lib/api";
import { PRACTITIONER_ID } from "@/lib/utils";
import type { PriorAuthResponse } from "@/lib/types";

type PageState = "input" | "loading" | "review";

function statusBadgeClass(status: string | undefined | null): string {
  if (!status) return "bg-amber-100 text-amber-800 border border-amber-300";
  switch (status.toLowerCase()) {
    case "approved":
      return "bg-green-100 text-green-800 border border-green-300";
    case "denied":
      return "bg-red-100 text-red-800 border border-red-200";
    case "submitted":
      return "bg-blue-100 text-blue-800 border border-blue-200";
    case "pending_approval":
    default:
      return "bg-amber-100 text-amber-800 border border-amber-300";
  }
}

function statusLabel(status: string | undefined | null): string {
  if (!status) return "Unknown";
  return status.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

function urgencyBadgeClass(urgency: string | undefined | null): string {
  if (!urgency) return "bg-slate-100 text-slate-700 border border-slate-200";
  switch (urgency.toLowerCase()) {
    case "urgent":
    case "stat":
      return "bg-red-100 text-red-800 border border-red-200";
    case "expedited":
      return "bg-amber-100 text-amber-800 border border-amber-200";
    default:
      return "bg-slate-100 text-slate-700 border border-slate-200";
  }
}

export default function PriorAuthPage() {
  const params = useParams<{ patientId: string }>();
  const searchParams = useSearchParams();
  const encounterId = searchParams.get("encounter") ?? params.patientId;

  // Form fields
  const [serviceDescription, setServiceDescription] = useState("");
  const [clinicalNotes, setClinicalNotes] = useState("");
  const [cptInput, setCptInput] = useState("");
  const [cptCodes, setCptCodes] = useState<string[]>([]);
  const [diagInput, setDiagInput] = useState("");
  const [diagCodes, setDiagCodes] = useState<string[]>([]);

  // State machine
  const [pageState, setPageState] = useState<PageState>("input");
  const [result, setResult] = useState<PriorAuthResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [approved, setApproved] = useState(false);
  const [showEvidence, setShowEvidence] = useState(false);

  // Tag input helpers
  function addCptCode() {
    const code = cptInput.trim().toUpperCase();
    if (code && !cptCodes.includes(code)) {
      setCptCodes((prev) => [...prev, code]);
    }
    setCptInput("");
  }

  function addDiagCode() {
    const code = diagInput.trim().toUpperCase();
    if (code && !diagCodes.includes(code)) {
      setDiagCodes((prev) => [...prev, code]);
    }
    setDiagInput("");
  }

  async function handleAssemble() {
    if (!serviceDescription.trim()) return;

    setError(null);
    setApproved(false);
    setPageState("loading");

    try {
      const res = await assemblePriorAuth(encounterId, {
        practitioner_id: PRACTITIONER_ID,
        // service_request_id is required by the API type — use encounter as the SR reference
        service_request_id: `sr-${encounterId}`,
        service_description: serviceDescription.trim(),
        diagnosis_codes: diagCodes,
        cpt_codes: cptCodes,
        clinical_notes: clinicalNotes.trim() || undefined,
        practitioner_role: "physician",
      });
      setResult(res);
      setPageState("review");
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to assemble PA package"
      );
      setPageState("input");
    }
  }

  function handleApproveSubmit() {
    setApproved(true);
    // In production: POST to payer API (CMS-0057 / X12 270/271)
    setTimeout(() => {
      setPageState("input");
      setServiceDescription("");
      setClinicalNotes("");
      setCptCodes([]);
      setDiagCodes([]);
      setResult(null);
      setApproved(false);
    }, 2500);
  }

  function handleCancel() {
    setPageState("input");
    setResult(null);
    setError(null);
  }

  const canAssemble = serviceDescription.trim().length > 0;

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="flex items-center gap-3 mb-5">
        <ClipboardList className="w-5 h-5 text-slate-600" />
        <h2 className="text-lg font-bold text-slate-900">Prior Authorization</h2>
        <span
          className={`px-2.5 py-0.5 text-xs font-semibold rounded-full ${
            pageState === "loading"
              ? "bg-amber-50 text-amber-700 border border-amber-200 animate-clinical-pulse"
              : pageState === "review"
              ? "bg-blue-50 text-blue-700 border border-blue-200"
              : "bg-slate-100 text-slate-600 border border-slate-200"
          }`}
        >
          {pageState === "loading"
            ? "Assembling…"
            : pageState === "review"
            ? "Review"
            : "Input"}
        </span>
      </div>

      {/* Approved flash */}
      {approved && (
        <div className="mb-4 flex items-center gap-3 p-3 bg-green-50 border border-green-200 rounded-lg text-green-800">
          <CheckCircle className="w-5 h-5 text-green-600" />
          <div>
            <p className="font-semibold text-sm">PA package approved and submitted</p>
            <p className="text-xs mt-0.5 text-green-700">
              FHIR ServiceRequest created · VERITAS audit trail updated ·
              Returning to input…
            </p>
          </div>
        </div>
      )}

      {/* Error display */}
      {error && pageState === "input" && (
        <div className="mb-4 flex items-start gap-3 p-3 bg-red-50 border border-red-200 rounded-lg text-red-800">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5 text-red-600" />
          <div>
            <p className="font-semibold text-sm">Assembly failed</p>
            <p className="text-xs mt-0.5 text-red-700">{error}</p>
          </div>
        </div>
      )}

      {/* ------------------------------------------------------------------ */}
      {/* INPUT STATE                                                         */}
      {/* ------------------------------------------------------------------ */}
      {(pageState === "input" || pageState === "loading") && (
        <div className="space-y-5 bg-white border border-slate-200 rounded-xl p-5">
          {/* Service description */}
          <div>
            <label className="block text-sm font-semibold text-slate-700 mb-1.5">
              Service Description{" "}
              <span className="text-red-500">*</span>
            </label>
            <textarea
              value={serviceDescription}
              onChange={(e) => setServiceDescription(e.target.value)}
              rows={3}
              placeholder="Describe the service requiring prior authorization, e.g., MRI lumbar spine without contrast, bariatric surgery evaluation, chemotherapy initiation"
              className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm text-slate-900 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent resize-none"
              disabled={pageState === "loading"}
            />
          </div>

          {/* CPT codes */}
          <div>
            <label className="block text-sm font-semibold text-slate-700 mb-1.5">
              CPT Codes
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={cptInput}
                onChange={(e) => setCptInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addCptCode()}
                placeholder="e.g., 72148"
                className="flex-1 px-3 py-2 border border-slate-300 rounded-lg text-sm font-clinical-mono text-slate-900 placeholder-slate-400 placeholder:font-sans focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                disabled={pageState === "loading"}
              />
              <button
                onClick={addCptCode}
                disabled={!cptInput.trim() || pageState === "loading"}
                className="flex items-center gap-1 px-3 py-2 bg-slate-100 text-slate-700 text-sm rounded-lg hover:bg-slate-200 disabled:opacity-40 transition-colors"
              >
                <Plus className="w-3.5 h-3.5" />
                Add
              </button>
            </div>
            {cptCodes.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {cptCodes.map((code) => (
                  <span
                    key={code}
                    className="inline-flex items-center gap-1 px-2 py-0.5 bg-blue-50 text-blue-700 border border-blue-200 text-xs font-clinical-mono rounded"
                  >
                    {code}
                    <button
                      onClick={() =>
                        setCptCodes((prev) => prev.filter((c) => c !== code))
                      }
                      className="hover:text-red-600 transition-colors"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* Diagnosis codes */}
          <div>
            <label className="block text-sm font-semibold text-slate-700 mb-1.5">
              Diagnosis Codes (ICD-10)
            </label>
            <div className="flex gap-2 mb-2">
              <input
                type="text"
                value={diagInput}
                onChange={(e) => setDiagInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && addDiagCode()}
                placeholder="e.g., M54.5"
                className="flex-1 px-3 py-2 border border-slate-300 rounded-lg text-sm font-clinical-mono text-slate-900 placeholder-slate-400 placeholder:font-sans focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                disabled={pageState === "loading"}
              />
              <button
                onClick={addDiagCode}
                disabled={!diagInput.trim() || pageState === "loading"}
                className="flex items-center gap-1 px-3 py-2 bg-slate-100 text-slate-700 text-sm rounded-lg hover:bg-slate-200 disabled:opacity-40 transition-colors"
              >
                <Plus className="w-3.5 h-3.5" />
                Add
              </button>
            </div>
            {diagCodes.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {diagCodes.map((code) => (
                  <span
                    key={code}
                    className="inline-flex items-center gap-1 px-2 py-0.5 bg-purple-50 text-purple-700 border border-purple-200 text-xs font-clinical-mono rounded"
                  >
                    {code}
                    <button
                      onClick={() =>
                        setDiagCodes((prev) => prev.filter((c) => c !== code))
                      }
                      className="hover:text-red-600 transition-colors"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* Clinical notes */}
          <div>
            <label className="block text-sm font-semibold text-slate-700 mb-1.5">
              Clinical Notes{" "}
              <span className="text-slate-400 font-normal">(optional)</span>
            </label>
            <textarea
              value={clinicalNotes}
              onChange={(e) => setClinicalNotes(e.target.value)}
              rows={4}
              placeholder="Additional clinical context, failed conservative treatments, relevant history, medical necessity rationale…"
              className="w-full px-3 py-2 border border-slate-300 rounded-lg text-sm text-slate-900 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent resize-none"
              disabled={pageState === "loading"}
            />
          </div>

          {/* Assemble button */}
          <div className="flex justify-end pt-1">
            <button
              onClick={handleAssemble}
              disabled={!canAssemble || pageState === "loading"}
              className="flex items-center gap-2 px-5 py-2.5 bg-blue-600 text-white text-sm font-semibold rounded-lg hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              {pageState === "loading" ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Assembling Package…
                </>
              ) : (
                <>
                  <ClipboardList className="w-4 h-4" />
                  Assemble PA Package
                </>
              )}
            </button>
          </div>
        </div>
      )}

      {/* ------------------------------------------------------------------ */}
      {/* REVIEW STATE                                                        */}
      {/* ------------------------------------------------------------------ */}
      {pageState === "review" && result && (
        <div className="space-y-5">
          {/* Status banner */}
          <div className="flex items-center justify-between p-4 bg-white border border-slate-200 rounded-xl">
            <div className="flex items-center gap-3">
              <FileText className="w-5 h-5 text-slate-600" />
              <div>
                <p className="font-semibold text-slate-900 text-sm">
                  PA Package Assembled
                </p>
                <p className="text-xs text-slate-500 mt-0.5">
                  Review the package below and approve to submit to payer
                </p>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <span
                className={`px-3 py-1 text-xs font-semibold rounded-full ${statusBadgeClass(result.prior_auth_status)}`}
              >
                {statusLabel(result.prior_auth_status)}
              </span>
              <span
                className={`px-2 py-0.5 text-xs font-semibold rounded ${urgencyBadgeClass(result.urgency)}`}
              >
                {result.urgency}
              </span>
            </div>
          </div>

          {/* Package content */}
          <div className="bg-white border border-slate-200 rounded-xl overflow-hidden">
            {/* Diagnosis summary */}
            <div className="p-5 border-b border-slate-100">
              <h4 className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-2">
                Diagnosis Summary
              </h4>
              <p className="text-sm text-slate-800 leading-relaxed">
                {result.diagnosis_summary}
              </p>
              {/* ICD-10 codes */}
              {result.icd10_codes.length > 0 && (
                <div className="flex flex-wrap gap-1.5 mt-2">
                  {result.icd10_codes.map((code) => (
                    <span
                      key={code}
                      className="px-2 py-0.5 bg-purple-50 text-purple-700 border border-purple-200 text-xs font-clinical-mono rounded"
                    >
                      {code}
                    </span>
                  ))}
                </div>
              )}
            </div>

            {/* Clinical justification */}
            <div className="p-5 border-b border-slate-100">
              <h4 className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-2">
                Clinical Justification
              </h4>
              <p className="text-sm text-slate-800 leading-relaxed whitespace-pre-wrap">
                {result.clinical_justification}
              </p>
            </div>

            {/* Supporting evidence */}
            {result.supporting_evidence.length > 0 && (
              <div className="p-5 border-b border-slate-100">
                <button
                  onClick={() => setShowEvidence(!showEvidence)}
                  className="flex items-center justify-between w-full"
                >
                  <h4 className="text-xs font-bold text-slate-400 uppercase tracking-widest">
                    Supporting Evidence ({result.supporting_evidence.length})
                  </h4>
                  {showEvidence ? (
                    <ChevronUp className="w-4 h-4 text-slate-400" />
                  ) : (
                    <ChevronDown className="w-4 h-4 text-slate-400" />
                  )}
                </button>
                {showEvidence && (
                  <ul className="mt-3 space-y-1.5">
                    {result.supporting_evidence.map((ev, i) => (
                      <li
                        key={i}
                        className="flex items-start gap-2 text-sm text-slate-700"
                      >
                        <span className="text-blue-500 mt-0.5 flex-shrink-0">
                          ·
                        </span>
                        {ev}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            )}

            {/* CPT codes */}
            {result.cpt_codes.length > 0 && (
              <div className="p-5 border-b border-slate-100">
                <h4 className="text-xs font-bold text-slate-400 uppercase tracking-widest mb-2">
                  CPT Codes
                </h4>
                <div className="flex flex-wrap gap-1.5">
                  {result.cpt_codes.map((code) => (
                    <span
                      key={code}
                      className="px-2 py-0.5 bg-blue-50 text-blue-700 border border-blue-200 text-xs font-clinical-mono rounded"
                    >
                      {code}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {/* VERITAS provenance */}
            <div className="p-4 bg-slate-50">
              <div className="flex flex-wrap gap-x-6 gap-y-1 text-xs">
                <span className="text-slate-400">
                  Audit:{" "}
                  <span className="font-clinical-mono text-slate-600">
                    {result.audit_event_id}
                  </span>
                </span>
                {result.spec_hash && (
                  <span className="text-slate-400">
                    Spec:{" "}
                    <span className="font-clinical-mono text-slate-600">
                      {result.spec_hash}
                    </span>
                  </span>
                )}
                <span className="text-slate-400">
                  Status:{" "}
                  <span className="text-slate-600">{result.status}</span>
                </span>
              </div>
            </div>
          </div>

          {/* Actions */}
          <div className="flex items-center justify-between pt-1">
            <button
              onClick={handleCancel}
              className="flex items-center gap-2 px-4 py-2 text-sm font-medium text-slate-600 bg-white border border-slate-200 rounded-lg hover:bg-slate-50 transition-colors"
            >
              <XCircle className="w-4 h-4" />
              Cancel
            </button>
            <button
              onClick={handleApproveSubmit}
              disabled={result.prior_auth_status === "denied"}
              className="flex items-center gap-2 px-5 py-2.5 bg-green-600 text-white text-sm font-semibold rounded-lg hover:bg-green-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              <CheckCircle className="w-4 h-4" />
              Approve &amp; Submit
            </button>
          </div>
        </div>
      )}

      <div className="mt-8 p-3 bg-amber-50 border border-amber-200 rounded text-amber-800 text-xs">
        <strong>MOCK DATA</strong> — PA assembly uses Claude API via VERITAS
        policy gate. No real payer submissions are made.
      </div>
    </div>
  );
}
