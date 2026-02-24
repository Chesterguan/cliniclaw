"use client";

/**
 * Review — Turn-based human-AI collaboration surface.
 *
 * This is the clinician's primary review queue. Every agent action
 * (note generation, order proposal, PA assembly) creates a Turn that
 * appears here for human review. The clinician can:
 *
 *   Accept   — approve AI output as-is
 *   Modify   — edit output and capture structured feedback (diff)
 *   Reject   — discard output with reason
 *   Escalate — flag for senior review
 *
 * A Workspace is auto-created when this page loads (idempotent — the API
 * returns the existing open workspace if one exists for the encounter).
 *
 * TurnQueue polls every 5s for new turns, so agents generating output
 * in other tabs appear here in near-real-time.
 */

import { useEffect, useState, useCallback } from "react";
import { useParams, useSearchParams } from "next/navigation";
import {
  MessageSquare,
  Loader2,
  AlertCircle,
  CheckCircle2,
  Clock,
  BarChart3,
  Lock,
} from "lucide-react";
import { createWorkspace } from "@/lib/api";
import { PRACTITIONER_ID } from "@/lib/utils";
import { TurnQueue } from "@/components/clinical/turn-queue";
import type { Workspace } from "@/lib/types";

export default function ReviewPage() {
  const params = useParams<{ patientId: string }>();
  const searchParams = useSearchParams();
  const encounterId = searchParams.get("encounter") ?? params.patientId;

  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const initWorkspace = useCallback(async () => {
    if (!encounterId) return;
    setLoading(true);
    setError(null);
    try {
      const ws = await createWorkspace(encounterId, PRACTITIONER_ID);
      setWorkspace(ws);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to initialize workspace"
      );
    } finally {
      setLoading(false);
    }
  }, [encounterId]);

  useEffect(() => {
    initWorkspace();
  }, [initWorkspace]);

  return (
    <div className="p-6 max-w-4xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-5">
        <div className="flex items-center gap-3">
          <MessageSquare className="w-5 h-5 text-slate-600" />
          <h2 className="text-lg font-bold text-slate-900">
            Review Queue
          </h2>
          {workspace && !workspace.closed_at && (
            <span className="px-2.5 py-0.5 text-xs font-semibold rounded-full bg-green-50 text-green-700 border border-green-200">
              Active
            </span>
          )}
          {workspace?.closed_at && (
            <span className="px-2.5 py-0.5 text-xs font-semibold rounded-full bg-slate-100 text-slate-600 border border-slate-200">
              Closed
            </span>
          )}
        </div>
        {workspace && (
          <div className="flex items-center gap-3 text-xs text-slate-500">
            <span className="flex items-center gap-1">
              <Clock className="w-3 h-3" />
              {new Date(workspace.created_at).toLocaleString()}
            </span>
            {workspace.pending_turns > 0 && (
              <span className="flex items-center gap-1.5 px-2 py-0.5 bg-amber-50 text-amber-700 border border-amber-200 rounded-full font-semibold">
                <BarChart3 className="w-3 h-3" />
                {workspace.pending_turns} pending
              </span>
            )}
          </div>
        )}
      </div>

      {/* Loading */}
      {loading && (
        <div className="flex items-center justify-center py-16">
          <Loader2 className="w-5 h-5 animate-spin text-slate-400 mr-3" />
          <span className="text-slate-500 text-sm">
            Initializing workspace...
          </span>
        </div>
      )}

      {/* Error */}
      {error && (
        <div className="mb-4 flex items-start gap-3 p-4 bg-red-50 border border-red-200 rounded-lg text-red-800">
          <AlertCircle className="w-5 h-5 flex-shrink-0 mt-0.5 text-red-600" />
          <div>
            <p className="font-semibold text-sm">Workspace error</p>
            <p className="text-xs mt-0.5 text-red-700">{error}</p>
            <button
              onClick={initWorkspace}
              className="mt-2 text-xs text-red-600 underline hover:text-red-800"
            >
              Retry
            </button>
          </div>
        </div>
      )}

      {/* Workspace active — show turn queue */}
      {workspace && !loading && (
        <div className="bg-slate-900 rounded-xl p-5">
          <TurnQueue workspaceId={workspace.id} />
        </div>
      )}

      {/* Empty state explanation */}
      {workspace && !loading && workspace.pending_turns === 0 && (
        <div className="mt-6 space-y-3">
          <div className="p-4 bg-white border border-slate-200 rounded-lg">
            <h3 className="text-sm font-semibold text-slate-700 mb-2 flex items-center gap-2">
              <CheckCircle2 className="w-4 h-4 text-green-500" />
              How Turn-Based Review Works
            </h3>
            <ul className="text-xs text-slate-600 space-y-1.5 ml-6 list-disc">
              <li>
                Generate a note in the <strong>Notes</strong> tab, propose an
                order in <strong>Orders</strong>, or assemble a PA package in{" "}
                <strong>Prior Auth</strong>
              </li>
              <li>
                Each agent action creates a <strong>Turn</strong> that appears
                here for your review
              </li>
              <li>
                <strong>Accept</strong> the AI output as-is, <strong>Edit</strong>{" "}
                it (captures structured feedback), <strong>Reject</strong> it, or{" "}
                <strong>Escalate</strong> for senior review
              </li>
              <li>
                All decisions are audited through the VERITAS trust layer
              </li>
            </ul>
          </div>

          <div className="p-3 bg-slate-50 border border-slate-200 rounded-lg flex items-start gap-2">
            <Lock className="w-3.5 h-3.5 text-slate-400 mt-0.5 flex-shrink-0" />
            <p className="text-xs text-slate-500">
              Workspace{" "}
              <span className="font-clinical-mono text-slate-600">
                {workspace.id.slice(0, 8)}...
              </span>{" "}
              is scoped to this encounter. All agent turns are linked to this
              workspace for audit traceability.
            </p>
          </div>
        </div>
      )}

      <div className="mt-8 p-3 bg-amber-50 border border-amber-200 rounded text-amber-800 text-xs">
        <strong>MOCK DATA</strong> — Agent turns are created when you use Notes,
        Orders, or Prior Auth tabs. Turn resolution is persisted to the local
        SQLite audit store.
      </div>
    </div>
  );
}
