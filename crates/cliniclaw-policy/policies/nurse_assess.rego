package cliniclaw.nurse_assess

# Nursing Assessment policies
# Translated from nurse_assess.toml [[rule]] sections
#
# Note: Rego allows OR via set membership — cleaner than 3 separate TOML rules.

default decision := "deny"

# Priority 20: deny nursing assessment on finished encounters
decision := "deny" if {
    input.action == "nurse_assess.evaluate"
    input.properties.encounter_status == "finished"
}

# Priority 10: allow nursing assessment across valid encounter states
# (in-progress, arrived, triaged)
decision := "allow" if {
    input.action == "nurse_assess.evaluate"
    "nurse_assess" in input.capabilities
    input.properties.encounter_status in {"in-progress", "arrived", "triaged"}
}
