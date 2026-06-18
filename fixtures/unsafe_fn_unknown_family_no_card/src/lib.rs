pub unsafe fn no_specific_operation_changed() {
    // No raw pointer dereference, transmute, or other body operation.
    // The declaration itself is classified as unsafe_declaration.
    // The owner card still emits a contract_missing card with a # Safety-doc action.
}
