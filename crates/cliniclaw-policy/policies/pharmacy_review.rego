package cliniclaw.pharmacy_review

# Pharmacy Review policies
# Translated from pharmacy_review.toml [[rule]] sections

default decision := "deny"

# Priority 20: deny pharmacy review on finished encounters
decision := "deny" if {
    input.action == "pharmacy_review.evaluate"
    input.properties.encounter_status == "finished"
}

# Priority 10: allow pharmacy review with capability + in-progress encounter
decision := "allow" if {
    input.action == "pharmacy_review.evaluate"
    "pharmacy_review" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
