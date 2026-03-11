package cliniclaw.prior_auth

# Prior Authorization policies
# Translated from prior_auth.toml [[rule]] sections

default decision := "deny"

# Priority 20: PA submission always requires approval (physician sign-off)
decision := "require_approval" if {
    input.action == "prior_auth.submit"
    "prior_auth" in input.capabilities
}

# Priority 10: allow PA assembly
decision := "allow" if {
    input.action == "prior_auth.assemble"
    "prior_auth" in input.capabilities
}

# Priority 10: allow PA status check
decision := "allow" if {
    input.action == "prior_auth.check_status"
    "prior_auth" in input.capabilities
}
