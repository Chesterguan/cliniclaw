"use client";

/**
 * PatientContext — persistent patient state across chart tab navigation.
 *
 * In a real HIS (Epic Storyboard, Cerner PowerChart), when you open a patient
 * chart the patient context is loaded once and persists as you switch between
 * Notes, Orders, Prior Auth, and Vitals tabs. This context replicates that
 * behavior: loadPatient() fetches from FHIR and the result is held in React
 * state for the lifetime of the chart session.
 *
 * PHI note: This context is in-memory only — no localStorage, no cookie
 * persistence. When the clinician closes the chart tab, the context is gone.
 */

import React, { createContext, useContext, useState, useCallback, useRef } from "react";
import type { PatientContext as PatientContextType, SafetyFlags } from "./types";
import { fetchPatient, fetchEncounter } from "./api";

// Re-export the type under a cleaner name for consumers
export type { PatientContext as PatientContextType } from "./types";

interface PatientContextValue {
  /** The currently-loaded patient context, or null if no chart is open. */
  context: PatientContextType | null;

  /** Directly set or clear the patient context (used internally by the chart layout). */
  setContext: (ctx: PatientContextType | null) => void;

  /** True while loadPatient() is in flight. */
  loading: boolean;

  /** Error string from the last loadPatient() call, if any. */
  error: string | null;

  /**
   * Fetch patient + encounter from the FHIR proxy and populate the context.
   * Safe to call multiple times — subsequent calls with the same IDs are
   * short-circuited if the context is already loaded for that patient+encounter.
   */
  loadPatient: (patientId: string, encounterId: string) => Promise<void>;
}

const PatientContext = createContext<PatientContextValue | null>(null);

export function PatientProvider({ children }: { children: React.ReactNode }) {
  const [context, setContext] = useState<PatientContextType | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Use a ref for the short-circuit check so loadPatient doesn't need
  // `context` in its dependency array (which would cause infinite re-renders).
  const contextRef = useRef(context);
  contextRef.current = context;

  const loadPatient = useCallback(
    async (patientId: string, encounterId: string) => {
      // Short-circuit: already loaded this patient+encounter.
      if (
        contextRef.current?.patient.id === patientId &&
        contextRef.current?.encounter.id === encounterId
      ) {
        return;
      }

      setLoading(true);
      setError(null);

      try {
        // Fetch both resources concurrently — independent FHIR reads.
        const [patient, encounter] = await Promise.all([
          fetchPatient(patientId),
          fetchEncounter(encounterId),
        ]);

        // Derive safety flags from the FHIR Patient resource.
        // deceased and inactive are the two flags surfaced in the banner.
        const flags: SafetyFlags = {
          deceased: patient.deceasedBoolean === true,
          inactive: patient.active === false,
        };

        // Build the context object. allergies, problemList, and activeMedications
        // come from the worklist endpoint which already fetches them. Here we
        // bootstrap with empty arrays — the chart layout will hydrate these from
        // the worklist entry if available, or they stay empty (shown as "None").
        const ctx: PatientContextType = {
          patient,
          encounter,
          allergies: [],
          problemList: [],
          activeMedications: [],
          flags,
        };

        setContext(ctx);
      } catch (err) {
        const msg = err instanceof Error ? err.message : "Failed to load patient";
        setError(msg);
      } finally {
        setLoading(false);
      }
    },
    []
  );

  return (
    <PatientContext.Provider
      value={{ context, setContext, loading, error, loadPatient }}
    >
      {children}
    </PatientContext.Provider>
  );
}

/**
 * usePatientContext — access the current patient context.
 *
 * Must be called from within a <PatientProvider> tree.
 * Throws if called outside the provider to fail loudly during development.
 */
export function usePatientContext(): PatientContextValue {
  const ctx = useContext(PatientContext);
  if (!ctx) {
    throw new Error("usePatientContext must be used within a PatientProvider");
  }
  return ctx;
}
