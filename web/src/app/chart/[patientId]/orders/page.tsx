"use client";

/**
 * CPOE (Computerized Physician Order Entry) — Orders tab.
 *
 * Implements the Order Basket pattern from Epic CPOE and Cerner PowerChart:
 *
 *   1. Clinician types a natural-language order ("metoprolol 25mg PO daily")
 *   2. "Parse Order" → API call → structured MedicationRequest returned
 *   3. CDS cards appear inline with the order (color-coded by indicator)
 *   4. Clinician can parse multiple orders into the basket
 *   5. "Sign All Orders" finalizes the basket
 *
 * CDS card indicator color mapping (per CDS Hooks spec):
 *   critical  → red   (would block if hard_stop)
 *   hard_stop → red   (requires explicit override)
 *   warning   → amber
 *   info      → blue
 *
 * The basket accumulates until signed. Orders can be removed individually.
 * Signing clears the basket and shows a confirmation.
 */

import { useState } from "react";
import { useParams, useSearchParams } from "next/navigation";
import {
  ShoppingCart,
  Plus,
  Trash2,
  AlertTriangle,
  AlertCircle,
  Info,
  CheckCircle,
  Loader2,
  XCircle,
  Pill,
} from "lucide-react";
import { proposeOrder } from "@/lib/api";
import { usePatientContext } from "@/lib/patient-context";
import { PRACTITIONER_ID } from "@/lib/utils";
import type { ProposeOrderResponse, CdsCard } from "@/lib/types";

// A basket item combines the API response with a local ID for list keying
interface BasketItem {
  localId: string;
  orderText: string;
  response: ProposeOrderResponse;
}

// Pull display fields from the MedicationRequest FHIR resource
function extractMedRequestDisplay(medReq: Record<string, unknown>): {
  medication: string;
  dose: string;
  route: string;
  frequency: string;
  status: string;
} {
  // FHIR MedicationRequest shape varies; we extract best-effort fields
  const medCodeableConcept = medReq.medicationCodeableConcept as
    | Record<string, unknown>
    | undefined;
  const coding = (
    medCodeableConcept?.coding as Array<Record<string, string>> | undefined
  )?.[0];

  const medication =
    (medCodeableConcept?.text as string) ||
    coding?.display ||
    "Unknown medication";

  // dosageInstruction[0]
  const dosageArr = medReq.dosageInstruction as
    | Array<Record<string, unknown>>
    | undefined;
  const dosage = dosageArr?.[0];

  const doseAndRate = (
    dosage?.doseAndRate as Array<Record<string, unknown>> | undefined
  )?.[0];
  const doseQty = doseAndRate?.doseQuantity as
    | Record<string, string | number>
    | undefined;
  const dose = doseQty
    ? `${doseQty.value} ${doseQty.unit ?? ""}`.trim()
    : "—";

  const routeCode = dosage?.route as Record<string, unknown> | undefined;
  const route = (routeCode?.text as string) || "—";

  const timing = dosage?.timing as Record<string, unknown> | undefined;
  const repeat = timing?.repeat as Record<string, unknown> | undefined;
  const frequency =
    (timing?.code as Record<string, string> | undefined)?.text ||
    (repeat ? `${repeat.frequency}x per ${repeat.period} ${repeat.periodUnit}` : "—");

  const status = (medReq.status as string) || "unknown";

  return { medication, dose, route, frequency, status };
}

export default function OrdersPage() {
  const params = useParams<{ patientId: string }>();
  const searchParams = useSearchParams();
  const encounterId = searchParams.get("encounter") ?? params.patientId;

  const { context } = usePatientContext();

  const [orderText, setOrderText] = useState("");
  const [parsing, setParsing] = useState(false);
  const [parseError, setParseError] = useState<string | null>(null);
  const [basket, setBasket] = useState<BasketItem[]>([]);
  const [signed, setSigned] = useState(false);

  async function handleParseOrder() {
    if (!orderText.trim()) return;

    setParsing(true);
    setParseError(null);
    setSigned(false);

    try {
      const res = await proposeOrder(encounterId, {
        practitioner_id: PRACTITIONER_ID,
        order_text: orderText.trim(),
        active_medications: context?.activeMedications ?? [],
        practitioner_role: "physician",
      });

      const item: BasketItem = {
        localId: `order-${Date.now()}-${Math.random().toString(36).slice(2)}`,
        orderText: orderText.trim(),
        response: res,
      };

      setBasket((prev) => [...prev, item]);
      setOrderText("");
    } catch (err) {
      setParseError(
        err instanceof Error ? err.message : "Failed to parse order"
      );
    } finally {
      setParsing(false);
    }
  }

  function handleRemoveOrder(localId: string) {
    setBasket((prev) => prev.filter((item) => item.localId !== localId));
  }

  function handleSignAll() {
    // In production: POST each MedicationRequest to FHIR.
    setSigned(true);
    setBasket([]);
    setTimeout(() => setSigned(false), 4000);
  }

  // Determine if signing should be blocked (any hard_stop CDS card)
  const hasHardStop = basket.some((item) =>
    item.response.cds_cards.some((c) => c.indicator === "hard_stop")
  );

  return (
    <div className="p-6 max-w-4xl mx-auto">
      <div className="flex items-center gap-3 mb-5">
        <ShoppingCart className="w-5 h-5 text-slate-600" />
        <h2 className="text-lg font-bold text-slate-900">Order Entry</h2>
        {basket.length > 0 && (
          <span className="px-2 py-0.5 bg-blue-100 text-blue-700 text-xs font-bold rounded-full">
            {basket.length} in basket
          </span>
        )}
      </div>

      {/* Signed confirmation */}
      {signed && (
        <div className="mb-4 flex items-center gap-3 p-3 bg-green-50 border border-green-200 rounded-lg text-green-800">
          <CheckCircle className="w-5 h-5 text-green-600" />
          <div>
            <p className="font-semibold text-sm">Orders signed and transmitted</p>
            <p className="text-xs mt-0.5 text-green-700">
              FHIR MedicationRequests created · VERITAS audit trail updated
            </p>
          </div>
        </div>
      )}

      {/* Order input */}
      <div className="bg-white border border-slate-200 rounded-xl p-5 mb-5">
        <h3 className="text-sm font-semibold text-slate-700 mb-3">
          Enter Order
        </h3>
        <div className="flex gap-3">
          <input
            type="text"
            value={orderText}
            onChange={(e) => setOrderText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !parsing) handleParseOrder();
            }}
            placeholder="e.g., metoprolol 25mg PO twice daily, lisinopril 10mg QD, CBC with diff STAT"
            className="flex-1 px-3 py-2 border border-slate-300 rounded-lg text-sm text-slate-900 placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
            disabled={parsing}
          />
          <button
            onClick={handleParseOrder}
            disabled={!orderText.trim() || parsing}
            className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white text-sm font-semibold rounded-lg hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors whitespace-nowrap"
          >
            {parsing ? (
              <>
                <Loader2 className="w-4 h-4 animate-spin" />
                Parsing…
              </>
            ) : (
              <>
                <Plus className="w-4 h-4" />
                Parse Order
              </>
            )}
          </button>
        </div>

        {parseError && (
          <div className="mt-3 flex items-center gap-2 p-2.5 bg-red-50 border border-red-200 rounded text-red-700 text-xs">
            <AlertCircle className="w-4 h-4 flex-shrink-0" />
            {parseError}
          </div>
        )}

        {/* Active medications context note */}
        {(context?.activeMedications.length ?? 0) > 0 && (
          <p className="text-xs text-slate-400 mt-2.5 flex items-center gap-1">
            <Pill className="w-3 h-3" />
            {context!.activeMedications.length} active medications included in
            drug interaction check
          </p>
        )}
      </div>

      {/* Order basket */}
      {basket.length > 0 && (
        <div className="space-y-4 mb-5">
          <h3 className="text-sm font-semibold text-slate-700">
            Order Basket
          </h3>
          {basket.map((item) => (
            <OrderCard
              key={item.localId}
              item={item}
              onRemove={() => handleRemoveOrder(item.localId)}
            />
          ))}
        </div>
      )}

      {/* Empty basket state */}
      {basket.length === 0 && !signed && (
        <div className="text-center py-12 text-slate-400 bg-white border border-dashed border-slate-200 rounded-xl">
          <ShoppingCart className="w-8 h-8 mx-auto mb-2 text-slate-300" />
          <p className="text-sm">Basket is empty</p>
          <p className="text-xs mt-1">
            Parse an order above to add it to the basket
          </p>
        </div>
      )}

      {/* Sign all button */}
      {basket.length > 0 && (
        <div className="flex items-center justify-between pt-2">
          {hasHardStop && (
            <div className="flex items-center gap-2 text-red-700 text-sm">
              <AlertTriangle className="w-4 h-4" />
              <span>Hard stop — resolve CDS alerts before signing</span>
            </div>
          )}
          <div className="ml-auto">
            <button
              onClick={handleSignAll}
              disabled={hasHardStop}
              className="flex items-center gap-2 px-6 py-2.5 bg-green-600 text-white text-sm font-semibold rounded-lg hover:bg-green-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
            >
              <CheckCircle className="w-4 h-4" />
              Sign All Orders ({basket.length})
            </button>
          </div>
        </div>
      )}

      <div className="mt-8 p-3 bg-amber-50 border border-amber-200 rounded text-amber-800 text-xs">
        <strong>MOCK DATA</strong> — Order parsing uses Claude API via VERITAS
        policy gate. No real prescriptions are generated.
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// OrderCard
// ---------------------------------------------------------------------------

function OrderCard({
  item,
  onRemove,
}: {
  item: BasketItem;
  onRemove: () => void;
}) {
  const { medication, dose, route, frequency, status } =
    extractMedRequestDisplay(
      item.response.medication_request as Record<string, unknown>
    );

  const hasCritical = item.response.cds_cards.some(
    (c) => c.indicator === "critical" || c.indicator === "hard_stop"
  );

  return (
    <div
      className={`bg-white border rounded-xl overflow-hidden ${
        hasCritical ? "border-red-300" : "border-slate-200"
      }`}
    >
      {/* Order header */}
      <div className="flex items-start justify-between p-4 pb-3">
        <div className="flex items-start gap-3">
          <Pill className="w-5 h-5 text-blue-500 mt-0.5 flex-shrink-0" />
          <div>
            <p className="font-semibold text-slate-900 text-sm">{medication}</p>
            <div className="flex flex-wrap gap-x-4 gap-y-0.5 mt-1 text-xs text-slate-500">
              <span>
                <span className="text-slate-400">Dose:</span> {dose}
              </span>
              <span>
                <span className="text-slate-400">Route:</span> {route}
              </span>
              <span>
                <span className="text-slate-400">Freq:</span> {frequency}
              </span>
              <span
                className={`px-1.5 py-0.5 rounded text-xs font-semibold ${
                  status === "active"
                    ? "bg-green-50 text-green-700"
                    : "bg-slate-100 text-slate-600"
                }`}
              >
                {status}
              </span>
            </div>
            <p className="text-xs text-slate-400 mt-1">
              From:{" "}
              <span className="italic">&ldquo;{item.orderText}&rdquo;</span>
            </p>
          </div>
        </div>
        <button
          onClick={onRemove}
          className="flex items-center gap-1 px-2 py-1 text-xs text-red-600 hover:bg-red-50 rounded transition-colors"
          title="Remove order"
        >
          <Trash2 className="w-3.5 h-3.5" />
          Remove
        </button>
      </div>

      {/* CDS Cards */}
      {item.response.cds_cards.length > 0 && (
        <div className="border-t border-slate-100 px-4 py-3 space-y-2">
          {item.response.cds_cards.map((card, idx) => (
            <CdsCardView key={idx} card={card} />
          ))}
        </div>
      )}

      {/* Audit trail footnote */}
      <div className="border-t border-slate-100 px-4 py-2 bg-slate-50">
        <p className="text-xs text-slate-400">
          Audit:{" "}
          <span className="font-clinical-mono">
            {item.response.audit_event_id}
          </span>
        </p>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// CDS Card View
// ---------------------------------------------------------------------------

function CdsCardView({ card }: { card: CdsCard }) {
  const config = {
    critical: {
      icon: <XCircle className="w-4 h-4" />,
      containerClass:
        "bg-red-50 border border-red-200 text-red-800",
      labelClass: "text-red-600 font-bold uppercase text-xs",
      label: "Critical",
    },
    hard_stop: {
      icon: <AlertTriangle className="w-4 h-4" />,
      containerClass:
        "bg-red-50 border border-red-300 text-red-800",
      labelClass: "text-red-700 font-bold uppercase text-xs",
      label: "Hard Stop",
    },
    warning: {
      icon: <AlertCircle className="w-4 h-4" />,
      containerClass:
        "bg-amber-50 border border-amber-200 text-amber-800",
      labelClass: "text-amber-600 font-bold uppercase text-xs",
      label: "Warning",
    },
    info: {
      icon: <Info className="w-4 h-4" />,
      containerClass:
        "bg-blue-50 border border-blue-200 text-blue-800",
      labelClass: "text-blue-600 font-bold uppercase text-xs",
      label: "Info",
    },
  };

  const c = config[card.indicator] ?? config.info;

  return (
    <div className={`flex items-start gap-3 p-3 rounded-lg ${c.containerClass}`}>
      <div className="flex-shrink-0 mt-0.5">{c.icon}</div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className={c.labelClass}>{c.label}</span>
          <span className="text-xs opacity-60">· {card.source}</span>
        </div>
        <p className="text-sm font-medium">{card.summary}</p>
        {card.detail && (
          <p className="text-xs mt-0.5 opacity-80">{card.detail}</p>
        )}
        {card.suggestions.length > 0 && (
          <div className="flex flex-wrap gap-1.5 mt-2">
            {card.suggestions.map((s, i) => (
              <span
                key={i}
                className="px-2 py-0.5 bg-white bg-opacity-60 border border-current border-opacity-20 text-xs rounded cursor-default"
                title={`Action: ${s.action_type}`}
              >
                {s.label}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
