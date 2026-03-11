package cliniclaw.triage_assess

# Triage Assessment policies
# Translated from triage_assess.toml [[rule]] sections

default decision := "deny"

# Priority 20: deny triage on finished encounters
decision := "deny" if {
    input.action == "triage_assess.evaluate"
    input.properties.encounter_status == "finished"
}

# Priority 10: allow triage assessment with capability + in-progress encounter
decision := "allow" if {
    input.action == "triage_assess.evaluate"
    "triage_assess" in input.capabilities
    input.properties.encounter_status == "in-progress"
}
