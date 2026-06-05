package cliniclaw.order_entry

default decision := "deny"

# Allow ordinary orders that carry the capability and are NOT renally risky.
decision := "allow" if {
    startswith(input.action, "order_entry.")
    "order_entry" in input.capabilities
    input.properties.egfr_low == "false"
}

# Deny renally-risky orders (low eGFR) — VERITAS blocks the contraindication.
decision := "deny" if {
    startswith(input.action, "order_entry.")
    input.properties.egfr_low == "true"
}
