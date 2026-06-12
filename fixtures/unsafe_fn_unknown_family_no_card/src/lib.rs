pub unsafe fn no_specific_operation_changed() {
    // No raw pointer dereference, transmute, or other classified operation.
    // This unsafe fn has no classified operation family — family will be Unknown.
    // In diff scope this card is suppressed as unclassified-family noise.
    // In repo scope this card is still emitted for inventory.
}
