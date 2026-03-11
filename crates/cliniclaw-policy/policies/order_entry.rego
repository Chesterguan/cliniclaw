package cliniclaw.order_entry

# Order Entry policies
# Translated from order_entry.toml [[rule]] sections

default decision := "deny"

# Priority 20: high-risk orders always require approval
decision := "require_approval" if {
    input.action == "order_entry.propose_high_risk"
    "order_entry" in input.capabilities
}

# Priority 10: allow standard order proposal
decision := "allow" if {
    input.action == "order_entry.propose"
    "order_entry" in input.capabilities
}

# Priority 10: allow order review
decision := "allow" if {
    input.action == "order_entry.review"
    "order_review" in input.capabilities
}
