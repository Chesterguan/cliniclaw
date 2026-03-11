package cliniclaw.lab_review

# Lab Review policies
# Translated from lab_review.toml [[rule]] sections

default decision := "deny"

# Priority 20: deny lab review on finished encounters
decision := "deny" if {
    input.action == "lab_review.interpret"
    input.properties.encounter_status == "finished"
}

# Priority 10: allow lab interpretation with capability + in-progress encounter
decision := "allow" if {
    input.action == "lab_review.interpret"
    "lab_review" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
