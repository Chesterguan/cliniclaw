package cliniclaw.order_entry

default decision := "deny"

# Allow ordinary orders that carry the capability and are not high-alert.
decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
    input.properties.high_alert == "false"
}

# Route HIGH-ALERT drug orders to human approval — a governance invariant,
# independent of any clinical contraindication check. VERITAS does not
# auto-apply these.
decision := "require_approval" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
    input.properties.high_alert == "true"
}
