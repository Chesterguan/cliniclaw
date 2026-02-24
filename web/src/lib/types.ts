// TS types matching the Rust API responses

export interface WorklistEntry {
  patient: WorklistPatient;
  encounter: WorklistEncounter;
  allergies: string[];
  problem_list: WorklistCondition[];
  active_medications_count: number;
  pending_orders_count: number;
  flags: SafetyFlags;
}

export interface WorklistPatient {
  id: string;
  name: string;
  birth_date: string | null;
  gender: string | null;
}

export interface WorklistEncounter {
  id: string;
  status: string;
  class_code: string;
  start_time: string | null;
  location: string | null;
}

export interface WorklistCondition {
  display: string;
  code: string;
}

export interface SafetyFlags {
  deceased: boolean;
  inactive: boolean;
}

export interface GenerateNoteRequest {
  practitioner_id: string;
  transcript: string;
  chief_complaint?: string;
  active_medications?: string[];
  practitioner_role?: string;
}

export interface GenerateNoteResponse {
  status: string;
  report: Record<string, unknown>;
  audit_event_id: string;
  spec_hash?: string;
  turn_id?: string;
  confidence: Confidence;
}

export interface ProposeOrderRequest {
  practitioner_id: string;
  order_text: string;
  active_medications?: string[];
  practitioner_role?: string;
}

export interface ProposeOrderResponse {
  status: string;
  medication_request: Record<string, unknown>;
  cds_cards: CdsCard[];
  audit_event_id: string;
  spec_hash?: string;
  turn_id?: string;
  confidence: Confidence;
}

export interface CdsCard {
  summary: string;
  detail?: string;
  indicator: "info" | "warning" | "critical" | "hard_stop";
  source: string;
  suggestions: CdsSuggestion[];
}

export interface CdsSuggestion {
  label: string;
  action_type: string;
}

export interface AssemblePriorAuthRequest {
  practitioner_id: string;
  service_request_id: string;
  service_description: string;
  diagnosis_codes: string[];
  cpt_codes: string[];
  clinical_notes?: string;
  practitioner_role?: string;
}

export interface PriorAuthResponse {
  status: string;
  diagnosis_summary: string;
  clinical_justification: string;
  supporting_evidence: string[];
  urgency: string;
  cpt_codes: string[];
  icd10_codes: string[];
  prior_auth_status: string;
  audit_event_id: string;
  spec_hash?: string;
  turn_id?: string;
  confidence: Confidence;
}

export interface AuditEvent {
  id: string;
  actor_id: string;
  patient_id: string | null;
  action: string;
  outcome: string;
  input_hash: string;
  output_hash: string;
  event_hash: string;
  previous_hash: string;
  timestamp: string;
}

export interface ChainVerification {
  valid: boolean;
  message: string;
}

export interface PatientContext {
  patient: FhirPatient;
  encounter: FhirEncounter;
  allergies: string[];
  problemList: WorklistCondition[];
  activeMedications: string[];
  flags: SafetyFlags;
}

export interface FhirPatient {
  resourceType: string;
  id: string;
  active?: boolean;
  name?: Array<{ family?: string; given?: string[] }>;
  gender?: string;
  birthDate?: string;
  deceasedBoolean?: boolean;
}

export interface FhirEncounter {
  resourceType: string;
  id: string;
  status: string;
  class?: { code?: string };
  subject?: { reference?: string; display?: string };
  participant?: Array<{ individual?: { reference?: string; display?: string } }>;
  period?: { start?: string; end?: string };
  location?: Array<{ location?: { display?: string } }>;
}

// ── Kernel types ──────────────────────────────────────────────────
export interface Workspace {
  id: string;
  encounter_id: string;
  practitioner_id: string;
  created_at: string;
  closed_at: string | null;
  pending_turns: number;
}

export interface Confidence {
  score: number;
  factors: string[];
}

export interface Feedback {
  action: 'accept' | 'modify' | 'reject' | 'escalate';
  corrected_output: Record<string, unknown> | null;
  reason: string | null;
}

export interface FeedbackStats {
  total_turns: number;
  accepted: number;
  modified: number;
  rejected: number;
  escalated: number;
  pending: number;
  avg_confidence: number;
}

export interface ReplayResult {
  turn_id: string;
  agent_name: string;
  input_snapshot: Record<string, unknown>;
  original_output: Record<string, unknown>;
  replay_output?: Record<string, unknown>;
  diff?: DiffEntry[];
  original_confidence?: Confidence;
  replay_confidence?: Confidence;
}

export interface DiffEntry {
  path: string;
  op: 'add' | 'remove' | 'replace';
  original?: unknown;
  replay?: unknown;
}

export interface Turn {
  id: string;
  workspace_id: string;
  agent_name: string;
  action: string;
  output_snapshot: Record<string, unknown>;
  confidence: Confidence;
  status: 'pending' | 'accepted' | 'modified' | 'rejected' | 'escalated';
  feedback: Feedback | null;
  created_at: string;
  resolved_at: string | null;
  resolved_by: string | null;
  triggered_by_turn_id?: string | null;
}
