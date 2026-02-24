import type {
  WorklistEntry,
  GenerateNoteRequest,
  GenerateNoteResponse,
  ProposeOrderRequest,
  ProposeOrderResponse,
  AssemblePriorAuthRequest,
  PriorAuthResponse,
  AuditEvent,
  ChainVerification,
  FhirPatient,
  FhirEncounter,
  Workspace,
  Turn,
  FeedbackStats,
  ReplayResult,
} from "./types";

const BASE = "/api";
const TOKEN = "demo-token"; // Mock mode token

async function request<T>(url: string, opts?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${url}`, {
    ...opts,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${TOKEN}`,
      ...opts?.headers,
    },
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }));
    throw new Error(body.error || `HTTP ${res.status}`);
  }

  return res.json();
}

// Worklist
// The API wraps entries in { practitioner_id, entries, total } — unwrap here
// so callers receive a flat WorklistEntry[] matching the WorklistEntry type.
export async function fetchWorklist(practitionerId: string): Promise<WorklistEntry[]> {
  const res = await request<{ entries: WorklistEntry[] }>(`/v1/worklist?practitioner_id=${practitionerId}`);
  return res.entries;
}

// FHIR proxy
export function fetchPatient(id: string): Promise<FhirPatient> {
  return request(`/v1/patients/${id}`);
}

export function fetchEncounter(id: string): Promise<FhirEncounter> {
  return request(`/v1/encounters/${id}`);
}

// Notes
export function generateNote(
  encounterId: string,
  body: GenerateNoteRequest
): Promise<GenerateNoteResponse> {
  return request(`/v1/encounter/${encounterId}/note`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

// Orders
export function proposeOrder(
  encounterId: string,
  body: ProposeOrderRequest
): Promise<ProposeOrderResponse> {
  return request(`/v1/encounter/${encounterId}/orders`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

// Prior Auth
export function assemblePriorAuth(
  encounterId: string,
  body: AssemblePriorAuthRequest
): Promise<PriorAuthResponse> {
  return request(`/v1/encounter/${encounterId}/prior-auth`, {
    method: "POST",
    body: JSON.stringify(body),
  });
}

// Audit
export function fetchAuditEvents(params: {
  patient_id?: string;
  action?: string;
}): Promise<AuditEvent[]> {
  const searchParams = new URLSearchParams();
  if (params.patient_id) searchParams.set("patient_id", params.patient_id);
  if (params.action) searchParams.set("action", params.action);
  return request(`/v1/audit/events?${searchParams}`);
}

export function fetchAuditEvent(id: string): Promise<AuditEvent> {
  return request(`/v1/audit/events/${id}`);
}

export function verifyAuditChain(): Promise<ChainVerification> {
  return request("/v1/audit/chain/verify");
}

// ── Kernel API ──────────────────────────────────────────────────
function get<T>(path: string): Promise<T> {
  return request(path);
}

function post<T>(path: string, body: unknown): Promise<T> {
  return request(path, { method: "POST", body: JSON.stringify(body) });
}

export function createWorkspace(encounter_id: string, practitioner_id: string): Promise<Workspace> {
  return post("/v1/workspaces", { encounter_id, practitioner_id });
}

export function getWorkspace(id: string): Promise<Workspace> {
  return get(`/v1/workspaces/${id}`);
}

export function closeWorkspace(id: string): Promise<Workspace> {
  return post(`/v1/workspaces/${id}/close`, {});
}

export function listTurns(workspaceId: string, status?: string): Promise<Turn[]> {
  const params = status ? `?status=${status}` : "";
  return get(`/v1/workspaces/${workspaceId}/turns${params}`);
}

export function getTurn(id: string): Promise<Turn> {
  return get(`/v1/turns/${id}`);
}

export function resolveTurn(
  id: string,
  body: {
    status: string;
    corrected_output?: Record<string, unknown>;
    reason?: string;
    resolved_by: string;
  }
): Promise<Turn> {
  return post(`/v1/turns/${id}/resolve`, body);
}

export function replayTurn(
  id: string,
  body?: { modified_input?: Record<string, unknown> }
): Promise<ReplayResult> {
  return post(`/v1/turns/${id}/replay`, body || {});
}

export function getTurnChain(id: string): Promise<Turn[]> {
  return get(`/v1/turns/${id}/chain`);
}

export function getFeedbackStats(agent_name?: string): Promise<FeedbackStats> {
  const params = agent_name ? `?agent_name=${agent_name}` : "";
  return get(`/v1/feedback/stats${params}`);
}
