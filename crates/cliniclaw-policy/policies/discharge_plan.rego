package cliniclaw.discharge_plan

# Discharge Plan policies
# Translated from discharge_plan.toml [[rule]] sections

default decision := "deny"

# Priority 20: deny discharge plan on finished encounters
decision := "deny" if {
    input.action == "discharge_plan.generate"
    input.properties.encounter_status == "finished"
}

# Priority 15: inpatient discharge requires approval
decision := "require_approval" if {
    input.action == "discharge_plan.generate"
    "discharge_plan" in input.capabilities
    input.properties.encounter_status == "in-progress"
    input.properties.encounter_class == "IMP"
}

# Priority 10: allow discharge plan generation (non-inpatient)
decision := "allow" if {
    input.action == "discharge_plan.generate"
    "discharge_plan" in input.capabilities
    input.properties.encounter_status == "in-progress"
    not input.properties.encounter_class == "IMP"
}
