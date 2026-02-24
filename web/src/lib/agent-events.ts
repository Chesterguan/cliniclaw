// Agent event types matching the Rust AgentEvent/AgentEventType

export type StepStatus = 'started' | 'completed' | 'failed';

export type AgentEventType =
  | { kind: 'agent_started' }
  | { kind: 'context_building'; step: number; detail: string }
  | { kind: 'skill_lookup'; skill_id: string | null; matched: boolean }
  | { kind: 'role_check'; role: string; allowed: boolean }
  | { kind: 'capability_check'; capability: string; valid: boolean }
  | { kind: 'population_gate'; passed: boolean; reason: string | null }
  | { kind: 'policy_evaluation'; decision: string; rule_name: string | null }
  | { kind: 'llm_call'; status: StepStatus; elapsed_ms: number | null }
  | { kind: 'response_parsing'; status: StepStatus; detail: string | null }
  | { kind: 'cds_check'; cards_count: number; max_severity: string | null }
  | { kind: 'verification'; passed: boolean; detail: string | null }
  | { kind: 'audit_creation'; audit_event_id: string }
  | { kind: 'fhir_write'; resource_type: string; resource_id: string | null }
  | { kind: 'turn_creation'; turn_id: string; confidence_score: number }
  | { kind: 'chain_trigger'; trigger_pattern: string; target_agent: string }
  | { kind: 'agent_completed'; confidence_score: number; elapsed_ms: number }
  | { kind: 'agent_failed'; error: string };

export interface AgentEvent {
  id: string;
  timestamp: string;
  encounter_id: string;
  workspace_id?: string;
  turn_id?: string;
  agent_name: string;
  event_type: AgentEventType;
  triggered_by_turn_id?: string;
}

// Labels for display
export const agentLabels: Record<string, string> = {
  ambient_doc: 'Ambient Documentation',
  order_entry: 'Order Entry',
  prior_auth: 'Prior Authorization',
};

export const eventKindLabels: Record<string, string> = {
  agent_started: 'Agent Started',
  context_building: 'Building Context',
  skill_lookup: 'Skill Lookup',
  role_check: 'Role Verification',
  capability_check: 'Capability Check',
  population_gate: 'Population Gate',
  policy_evaluation: 'Policy Evaluation',
  llm_call: 'Calling Claude',
  response_parsing: 'Parsing Response',
  cds_check: 'CDS Check',
  verification: 'Output Verification',
  audit_creation: 'Creating Audit Record',
  fhir_write: 'Writing to FHIR',
  turn_creation: 'Creating Turn',
  chain_trigger: 'Chain Triggered',
  agent_completed: 'Agent Completed',
  agent_failed: 'Agent Failed',
};

// Map event kinds to VERITAS governance stages
export type GovernanceStage = 'state' | 'policy' | 'capability' | 'execution' | 'verify' | 'audit';

export const eventToStage: Record<string, GovernanceStage> = {
  agent_started: 'state',
  context_building: 'state',
  skill_lookup: 'policy',
  role_check: 'policy',
  population_gate: 'policy',
  policy_evaluation: 'policy',
  capability_check: 'capability',
  llm_call: 'execution',
  response_parsing: 'execution',
  cds_check: 'execution',
  verification: 'verify',
  audit_creation: 'audit',
  fhir_write: 'audit',
  turn_creation: 'audit',
  agent_completed: 'audit',
  agent_failed: 'audit',
};
