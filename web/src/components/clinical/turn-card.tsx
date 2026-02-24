'use client';

import { useState } from 'react';
import { Turn } from '@/lib/types';
import { resolveTurn } from '@/lib/api';
import { ConfidenceMeter } from './confidence-meter';
import { useConfidenceUI } from '@/hooks/use-confidence-ui';
import { ReplayView } from './replay-view';
import { PRACTITIONER_ID } from '@/lib/utils';

const agentLabels: Record<string, string> = {
  ambient_doc: 'Ambient Documentation',
  order_entry: 'Order Entry',
  prior_auth: 'Prior Authorization',
};

const actionLabels: Record<string, string> = {
  generate_note: 'Generated Note',
  propose_order: 'Proposed Order',
  assemble_package: 'PA Package',
};

export function TurnCard({ turn, onResolved }: { turn: Turn; onResolved?: () => void }) {
  const [resolving, setResolving] = useState(false);
  const [editMode, setEditMode] = useState(false);
  const [editedOutput, setEditedOutput] = useState('');
  const [reason, setReason] = useState('');
  const [showReplay, setShowReplay] = useState(false);
  const cui = useConfidenceUI(turn.confidence);

  // Expand details by default for low-confidence turns
  const [detailsExpanded, setDetailsExpanded] = useState(cui.expandDetailsByDefault);

  const handleResolve = async (status: string) => {
    setResolving(true);
    try {
      const body: Record<string, unknown> = { status, resolved_by: PRACTITIONER_ID };
      if (status === 'modified' && editedOutput) {
        try {
          body.corrected_output = JSON.parse(editedOutput);
        } catch {
          body.corrected_output = { text: editedOutput };
        }
      }
      if (reason) body.reason = reason;
      await resolveTurn(turn.id, body as Parameters<typeof resolveTurn>[1]);
      onResolved?.();
    } finally {
      setResolving(false);
      setEditMode(false);
    }
  };

  const isChained = !!turn.triggered_by_turn_id;

  return (
    <div>
      <div className={`border rounded-lg p-4 ${cui.borderColor} ${cui.bgColor}`}>
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2">
            {isChained && (
              <span className="text-[10px] px-1.5 py-0.5 bg-purple-500/20 text-purple-400 rounded-full">
                chain
              </span>
            )}
            <span className="text-sm font-medium text-slate-200">
              {agentLabels[turn.agent_name] || turn.agent_name}
            </span>
            <span className="text-xs text-slate-500">
              {actionLabels[turn.action] || turn.action}
            </span>
          </div>
          <div className="flex items-center gap-2">
            {cui.badgeText && (
              <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${cui.badgeClass}`}>
                {cui.badgeText}
              </span>
            )}
            <ConfidenceMeter confidence={turn.confidence} />
            <span className={`text-xs px-2 py-0.5 rounded-full ${
              turn.status === 'pending' ? 'bg-amber-500/20 text-amber-400' :
              turn.status === 'accepted' ? 'bg-green-500/20 text-green-400' :
              turn.status === 'modified' ? 'bg-blue-500/20 text-blue-400' :
              turn.status === 'rejected' ? 'bg-red-500/20 text-red-400' :
              'bg-orange-500/20 text-orange-400'
            }`}>{turn.status}</span>
          </div>
        </div>

        {/* Collapsible output preview */}
        <button
          className="w-full text-left"
          onClick={() => setDetailsExpanded(!detailsExpanded)}
        >
          <div className="flex items-center gap-1 text-xs text-slate-500 mb-1">
            <svg
              className={`w-3 h-3 transition-transform ${detailsExpanded ? 'rotate-90' : ''}`}
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M9 18l6-6-6-6" />
            </svg>
            Output details
          </div>
        </button>

        {detailsExpanded && (
          <div className="bg-slate-800/50 rounded p-3 mb-3 text-sm text-slate-300 max-h-48 overflow-y-auto">
            <pre className="whitespace-pre-wrap font-mono text-xs">
              {JSON.stringify(turn.output_snapshot, null, 2)}
            </pre>
          </div>
        )}

        {/* Low confidence warning */}
        {cui.forceFullReview && turn.status === 'pending' && (
          <div className="flex items-center gap-2 p-2 mb-3 bg-amber-500/10 border border-amber-500/30 rounded text-xs text-amber-400">
            <svg className="w-3.5 h-3.5 flex-shrink-0" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 9v2m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            Low confidence — please review all details before accepting
          </div>
        )}

        {/* Edit mode */}
        {editMode && (
          <div className="mb-3 space-y-2">
            <textarea
              className="w-full bg-slate-800 border border-slate-600 rounded p-2 text-sm text-slate-200 font-mono"
              rows={6}
              value={editedOutput || JSON.stringify(turn.output_snapshot, null, 2)}
              onChange={(e) => setEditedOutput(e.target.value)}
              placeholder="Edit the output..."
            />
            <input
              className="w-full bg-slate-800 border border-slate-600 rounded p-2 text-sm text-slate-200"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              placeholder="Reason for modification (optional)"
            />
          </div>
        )}

        {/* Feedback display for resolved turns */}
        {turn.feedback && (
          <div className="mb-3 p-2 bg-slate-800/30 rounded text-xs text-slate-400">
            <span className="font-medium">Feedback:</span> {turn.feedback.action}
            {turn.feedback.reason && <span> — {turn.feedback.reason}</span>}
          </div>
        )}

        {/* Action buttons (only for pending turns) */}
        {turn.status === 'pending' && (
          <div className="flex gap-2 flex-wrap">
            {cui.showQuickAccept ? (
              <button
                onClick={() => handleResolve('accepted')}
                disabled={resolving}
                className="px-3 py-1.5 bg-green-600 hover:bg-green-500 text-white text-sm rounded disabled:opacity-50"
              >
                Quick Accept
              </button>
            ) : (
              <button
                onClick={() => handleResolve('accepted')}
                disabled={resolving || (cui.forceFullReview && !detailsExpanded)}
                className="px-3 py-1.5 bg-green-600 hover:bg-green-500 text-white text-sm rounded disabled:opacity-50"
                title={cui.forceFullReview && !detailsExpanded ? 'Expand details first' : ''}
              >
                Accept
              </button>
            )}
            {editMode ? (
              <button
                onClick={() => handleResolve('modified')}
                disabled={resolving}
                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white text-sm rounded disabled:opacity-50"
              >
                Save Changes
              </button>
            ) : (
              <button
                onClick={() => setEditMode(true)}
                className="px-3 py-1.5 bg-blue-600 hover:bg-blue-500 text-white text-sm rounded"
              >
                Edit & Accept
              </button>
            )}
            <button
              onClick={() => handleResolve('rejected')}
              disabled={resolving}
              className="px-3 py-1.5 bg-red-600 hover:bg-red-500 text-white text-sm rounded disabled:opacity-50"
            >
              Reject
            </button>
            <button
              onClick={() => handleResolve('escalated')}
              disabled={resolving}
              className="px-3 py-1.5 bg-orange-600 hover:bg-orange-500 text-white text-sm rounded disabled:opacity-50"
            >
              Escalate
            </button>
          </div>
        )}

        {/* Replay button for resolved turns */}
        {turn.status !== 'pending' && (
          <div className="flex gap-2 mt-2">
            <button
              onClick={() => setShowReplay(!showReplay)}
              className="px-3 py-1 text-xs bg-slate-700 hover:bg-slate-600 text-slate-300 rounded transition-colors"
            >
              {showReplay ? 'Hide Replay' : 'Replay'}
            </button>
          </div>
        )}

        <div className="mt-2 text-xs text-slate-500">
          {new Date(turn.created_at).toLocaleString()}
          {turn.resolved_by && ` · Resolved by ${turn.resolved_by}`}
        </div>
      </div>

      {/* Replay view inline below card */}
      {showReplay && <ReplayView turnId={turn.id} />}
    </div>
  );
}
