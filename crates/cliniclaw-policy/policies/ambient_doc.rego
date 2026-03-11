package cliniclaw.ambient_doc

# Ambient Documentation policies
# Translated from ambient_doc.toml [[rule]] sections

default decision := "deny"

# Priority 20: deny note generation on finished encounters
decision := "deny" if {
    input.action == "ambient_doc.generate_note"
    input.properties.encounter_status == "finished"
}

# Priority 10: allow note generation with capability + in-progress encounter
decision := "allow" if {
    input.action == "ambient_doc.generate_note"
    "note_generation" in input.capabilities
    input.properties.encounter_status == "in-progress"
}

# Priority 10: allow note review with capability
decision := "allow" if {
    input.action == "ambient_doc.review_note"
    "note_review" in input.capabilities
}
