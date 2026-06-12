pub unsafe fn no_specific_operation_changed() {
    // No raw pointer dereference, transmute, or other classified operation.
    // This unsafe fn has no classified operation family — family will be Unknown.
    // An unknown-family unsafe fn still emits a contract_missing card; the owner
    // card is actionable (next_action: add # Safety docs) in all scopes.
}
