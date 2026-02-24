use serde::{Deserialize, Serialize};

/// CDS Hooks-inspired alert card returned by agents.
///
/// Tiered indicators follow clinical decision support best practices:
/// - `Info`: informational, no action required
/// - `Warning`: requires attention, can be overridden with reason
/// - `Critical`: must acknowledge before proceeding
/// - `HardStop`: action blocked, cannot proceed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdsCard {
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub indicator: CdsIndicator,
    pub source: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub suggestions: Vec<CdsSuggestion>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CdsIndicator {
    Info,
    Warning,
    Critical,
    HardStop,
}

impl std::fmt::Display for CdsIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Critical => write!(f, "critical"),
            Self::HardStop => write!(f, "hard_stop"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CdsSuggestion {
    pub label: String,
    pub action_type: String,
}

impl CdsCard {
    pub fn info(summary: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            detail: None,
            indicator: CdsIndicator::Info,
            source: source.into(),
            suggestions: Vec::new(),
        }
    }

    pub fn warning(summary: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            detail: None,
            indicator: CdsIndicator::Warning,
            source: source.into(),
            suggestions: vec![
                CdsSuggestion {
                    label: "Override with reason".to_string(),
                    action_type: "override".to_string(),
                },
                CdsSuggestion {
                    label: "Remove order".to_string(),
                    action_type: "cancel".to_string(),
                },
            ],
        }
    }

    pub fn critical(summary: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            detail: None,
            indicator: CdsIndicator::Critical,
            source: source.into(),
            suggestions: vec![CdsSuggestion {
                label: "Acknowledge".to_string(),
                action_type: "accept".to_string(),
            }],
        }
    }

    pub fn hard_stop(summary: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            detail: None,
            indicator: CdsIndicator::HardStop,
            source: source.into(),
            suggestions: vec![CdsSuggestion {
                label: "Cancel order".to_string(),
                action_type: "cancel".to_string(),
            }],
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Check for common drug-drug interactions (hardcoded for demo).
///
/// Returns CDS cards for known interactions between the proposed medication
/// and the patient's existing medications.
pub fn check_drug_interactions(
    proposed_medication: &str,
    existing_medications: &[String],
) -> Vec<CdsCard> {
    let proposed = proposed_medication.to_lowercase();
    let mut cards = Vec::new();

    for med in existing_medications {
        let existing = med.to_lowercase();

        // Warfarin + NSAID interaction
        if (proposed.contains("warfarin") && is_nsaid(&existing))
            || (is_nsaid(&proposed) && existing.contains("warfarin"))
        {
            cards.push(
                CdsCard::warning(
                    format!("Drug interaction: {} + {}", proposed_medication, med),
                    "cliniclaw-policy",
                )
                .with_detail(
                    "Concurrent use of warfarin with NSAIDs increases bleeding risk. \
                     Consider gastroprotective agent or alternative analgesic.",
                ),
            );
        }

        // Metformin + contrast dye (common interaction check)
        if proposed.contains("metformin") && existing.contains("contrast")
            || proposed.contains("contrast") && existing.contains("metformin")
        {
            cards.push(
                CdsCard::critical(
                    format!("Drug interaction: {} + {}", proposed_medication, med),
                    "cliniclaw-policy",
                )
                .with_detail(
                    "Metformin should be held 48 hours before and after IV contrast \
                     administration due to risk of lactic acidosis.",
                ),
            );
        }

        // ACE inhibitor + potassium-sparing diuretic
        if (is_ace_inhibitor(&proposed) && is_k_sparing(&existing))
            || (is_k_sparing(&proposed) && is_ace_inhibitor(&existing))
        {
            cards.push(
                CdsCard::warning(
                    format!("Drug interaction: {} + {}", proposed_medication, med),
                    "cliniclaw-policy",
                )
                .with_detail("Risk of hyperkalemia with concurrent ACE inhibitor and potassium-sparing diuretic use."),
            );
        }
    }

    cards
}

/// Check if a medication is a high-risk medication requiring extra approval.
pub fn check_high_risk(medication: &str) -> Option<CdsCard> {
    let med = medication.to_lowercase();

    if med.contains("warfarin") || med.contains("heparin") || med.contains("enoxaparin") {
        Some(
            CdsCard::critical(
                format!("High-risk medication: {}", medication),
                "cliniclaw-policy",
            )
            .with_detail("Anticoagulant — requires physician approval and monitoring plan."),
        )
    } else if med.contains("insulin") {
        Some(
            CdsCard::warning(
                format!("High-risk medication: {}", medication),
                "cliniclaw-policy",
            )
            .with_detail("Insulin — verify dose, route, and frequency. High-alert medication."),
        )
    } else if med.contains("opioid")
        || med.contains("morphine")
        || med.contains("fentanyl")
        || med.contains("oxycodone")
        || med.contains("hydromorphone")
    {
        Some(
            CdsCard::critical(
                format!("Controlled substance: {}", medication),
                "cliniclaw-policy",
            )
            .with_detail("Opioid — requires DEA authorization, PDMP check, and informed consent."),
        )
    } else {
        None
    }
}

/// Check for duplicate orders.
pub fn check_duplicate(
    proposed_medication: &str,
    existing_medications: &[String],
) -> Option<CdsCard> {
    let proposed = proposed_medication.to_lowercase();
    for med in existing_medications {
        if med.to_lowercase().contains(&proposed) || proposed.contains(&med.to_lowercase()) {
            return Some(
                CdsCard::warning(
                    format!("Possible duplicate: {} already active", med),
                    "cliniclaw-agents",
                )
                .with_detail(
                    "Patient already has an active order for a similar medication. \
                     Verify this is not a duplicate.",
                ),
            );
        }
    }
    None
}

fn is_nsaid(med: &str) -> bool {
    med.contains("naproxen")
        || med.contains("ibuprofen")
        || med.contains("aspirin")
        || med.contains("celecoxib")
        || med.contains("diclofenac")
        || med.contains("ketorolac")
        || med.contains("meloxicam")
}

fn is_ace_inhibitor(med: &str) -> bool {
    med.contains("lisinopril")
        || med.contains("enalapril")
        || med.contains("ramipril")
        || med.contains("captopril")
        || med.contains("benazepril")
}

fn is_k_sparing(med: &str) -> bool {
    med.contains("spironolactone")
        || med.contains("eplerenone")
        || med.contains("amiloride")
        || med.contains("triamterene")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_warfarin_nsaid_interaction() {
        let cards = check_drug_interactions(
            "Warfarin 5mg",
            &["Naproxen 500mg".to_string()],
        );
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].indicator, CdsIndicator::Warning);
        assert!(cards[0].summary.contains("Warfarin"));
    }

    #[test]
    fn test_no_interaction() {
        let cards = check_drug_interactions(
            "Metformin 500mg",
            &["Lisinopril 10mg".to_string()],
        );
        assert!(cards.is_empty());
    }

    #[test]
    fn test_high_risk_warfarin() {
        let card = check_high_risk("Warfarin 5mg");
        assert!(card.is_some());
        assert_eq!(card.unwrap().indicator, CdsIndicator::Critical);
    }

    #[test]
    fn test_not_high_risk() {
        assert!(check_high_risk("Metformin 500mg").is_none());
    }

    #[test]
    fn test_duplicate_detection() {
        let card = check_duplicate(
            "metformin",
            &["Metformin 1000mg".to_string()],
        );
        assert!(card.is_some());
        assert_eq!(card.unwrap().indicator, CdsIndicator::Warning);
    }

    #[test]
    fn test_no_duplicate() {
        let card = check_duplicate(
            "lisinopril",
            &["Metformin 1000mg".to_string()],
        );
        assert!(card.is_none());
    }

    #[test]
    fn test_cds_card_serde_roundtrip() {
        let card = CdsCard::warning("Test warning", "test-source")
            .with_detail("Test detail");
        let json = serde_json::to_string(&card).unwrap();
        let deserialized: CdsCard = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.indicator, CdsIndicator::Warning);
        assert_eq!(deserialized.suggestions.len(), 2);
    }
}
