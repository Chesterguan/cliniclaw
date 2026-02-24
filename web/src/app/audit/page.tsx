/**
 * Standalone Audit Trail page — global view across all patients.
 *
 * Accessible from the sidebar nav. Shows all audit events with full
 * chain verification capability. Uses the shared AuditView component
 * which handles filtering, loading, and the hash chain detail expansion.
 *
 * This is a server component wrapper — AuditView itself is a client component.
 */

import { AuditView } from "@/components/audit-view";

export default function AuditPage() {
  return (
    <div className="p-6 max-w-4xl mx-auto">
      <AuditView showChainVerify={true} />

      <div className="mt-8 p-3 bg-amber-50 border border-amber-200 rounded text-amber-800 text-xs">
        <strong>MOCK DATA</strong> — Audit events reflect real VERITAS-model
        hash chaining. Event hashes and chain verification are cryptographically
        computed by the Rust backend.
      </div>
    </div>
  );
}
