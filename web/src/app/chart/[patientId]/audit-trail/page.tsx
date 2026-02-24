"use client";

/**
 * Chart-scoped Audit Trail — shows audit events for the current patient.
 *
 * This is the in-chart audit view, pre-filtered to the current patient.
 * It reuses the same shared AuditTimeline component as the standalone
 * /audit page, but scoped to one patient context.
 */

import { useParams } from "next/navigation";
import { AuditView } from "@/components/audit-view";

export default function ChartAuditPage() {
  const params = useParams<{ patientId: string }>();

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <AuditView initialPatientId={params.patientId} showChainVerify={false} />
    </div>
  );
}
