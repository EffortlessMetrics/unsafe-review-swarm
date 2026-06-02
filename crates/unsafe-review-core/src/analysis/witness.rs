use crate::domain::{HazardKind, WitnessKind, WitnessRoute};

pub(crate) fn routes_for(hazards: &[HazardKind], owner: Option<&String>) -> Vec<WitnessRoute> {
    if hazards.iter().any(|h| {
        matches!(
            h,
            HazardKind::SendSyncInvariant
                | HazardKind::AtomicOrdering
                | HazardKind::StaticMutGlobalState
        )
    }) {
        return vec![
            WitnessRoute {
                kind: WitnessKind::Loom,
                reason: "Concurrency or shared-mutable-state hazard; interleaving model is a better first witness than Miri alone".to_string(),
                command: owner.map(|name| format!("cargo test {name}_loom -- --nocapture")),
                required: false,
            },
            WitnessRoute {
                kind: WitnessKind::Shuttle,
                reason: "Shuttle can explore scheduler interleavings for focused concurrency invariants".to_string(),
                command: owner.map(|name| format!("cargo test {name}_shuttle -- --nocapture")),
                required: false,
            },
        ];
    }
    if hazards
        .iter()
        .any(|h| matches!(h, HazardKind::FfiAbi | HazardKind::FfiOwnership))
    {
        return vec![
            WitnessRoute {
                kind: WitnessKind::AddressSanitizer,
                reason: "FFI boundary detected; sanitizer/cargo-careful is usually more suitable than Miri for foreign calls".to_string(),
                command: Some(owner.map_or_else(
                    || "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test".to_string(),
                    |name| format!("RUSTFLAGS='-Z sanitizer=address' cargo +nightly test {name}"),
                )),
                required: false,
            },
            WitnessRoute {
                kind: WitnessKind::CargoCareful,
                reason: "cargo-careful can cheaply exercise Rust-side FFI boundary assumptions when a focused test exists".to_string(),
                command: Some(owner.map_or_else(
                    || "cargo +nightly careful test".to_string(),
                    |name| format!("cargo +nightly careful test {name}"),
                )),
                required: false,
            },
            WitnessRoute {
                kind: WitnessKind::HumanDeepReview,
                reason: "Reviewed FFI ABI and ownership seams can be recorded with a human deep-review receipt when executable witnesses cannot cross the foreign boundary".to_string(),
                command: None,
                required: false,
            },
        ];
    }
    if hazards.iter().any(|h| {
        matches!(
            h,
            HazardKind::InvalidValue
                | HazardKind::InitializedMemory
                | HazardKind::Alignment
                | HazardKind::PointerValidity
                | HazardKind::Bounds
                | HazardKind::AliasingOrProvenance
                | HazardKind::DropOrDeallocation
        )
    }) {
        return vec![WitnessRoute {
            kind: WitnessKind::Miri,
            reason: "Pure-Rust UB-adjacent hazard; Miri is the strongest concrete-execution witness when the path is supported".to_string(),
            command: owner.map(|name| format!("cargo +nightly miri test {name}")),
            required: false,
        }, WitnessRoute {
            kind: WitnessKind::CargoCareful,
            reason: "cargo-careful is a cheaper compatibility-oriented runtime check for several unsafe preconditions".to_string(),
            command: owner.map(|name| format!("cargo +nightly careful test {name}")),
            required: false,
        }];
    }
    vec![WitnessRoute {
        kind: WitnessKind::HumanDeepReview,
        reason: "The analyzer could not infer a precise witness route; use manual unsafe contract review".to_string(),
        command: None,
        required: false,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_rust_ub_hazards_route_to_miri_and_careful() {
        let owner = "read_header".to_string();
        let routes = routes_for(&[HazardKind::Alignment], Some(&owner));

        assert_route(
            &routes,
            WitnessKind::Miri,
            "cargo +nightly miri test read_header",
        );
        assert_route(
            &routes,
            WitnessKind::CargoCareful,
            "cargo +nightly careful test read_header",
        );
        assert_no_route(&routes, WitnessKind::AddressSanitizer);
    }

    #[test]
    fn drop_deallocation_hazards_route_to_miri_and_careful() {
        let owner = "drop_tail".to_string();
        let routes = routes_for(&[HazardKind::DropOrDeallocation], Some(&owner));

        assert_route(
            &routes,
            WitnessKind::Miri,
            "cargo +nightly miri test drop_tail",
        );
        assert_route(
            &routes,
            WitnessKind::CargoCareful,
            "cargo +nightly careful test drop_tail",
        );
    }

    #[test]
    fn ffi_hazards_route_to_sanitizer_and_careful_not_miri() {
        let routes = routes_for(&[HazardKind::FfiAbi, HazardKind::FfiOwnership], None);

        assert_route(
            &routes,
            WitnessKind::AddressSanitizer,
            "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test",
        );
        assert_route(
            &routes,
            WitnessKind::CargoCareful,
            "cargo +nightly careful test",
        );
        assert!(routes.iter().any(|route| {
            route.kind == WitnessKind::HumanDeepReview
                && route.command.is_none()
                && route.reason.contains("Reviewed FFI ABI")
        }));
        assert_no_route(&routes, WitnessKind::Miri);
    }

    #[test]
    fn concurrency_hazards_route_to_loom_and_shuttle_not_miri() {
        let owner = "SharedCell".to_string();
        let routes = routes_for(&[HazardKind::SendSyncInvariant], Some(&owner));

        assert_route(
            &routes,
            WitnessKind::Loom,
            "cargo test SharedCell_loom -- --nocapture",
        );
        assert_route(
            &routes,
            WitnessKind::Shuttle,
            "cargo test SharedCell_shuttle -- --nocapture",
        );
        assert_no_route(&routes, WitnessKind::Miri);
    }

    #[test]
    fn unsupported_or_precise_unknown_hazards_route_to_human_review() {
        let routes = routes_for(&[HazardKind::PinInvariant], None);

        assert!(routes.iter().any(|route| {
            route.kind == WitnessKind::HumanDeepReview
                && route.reason.contains("manual unsafe contract review")
                && route.command.is_none()
        }));
    }

    fn assert_route(routes: &[WitnessRoute], kind: WitnessKind, command: &str) {
        assert!(
            routes.iter().any(|route| {
                route.kind == kind
                    && route.command.as_deref() == Some(command)
                    && !route.reason.is_empty()
            }),
            "expected route {kind:?} with command {command}"
        );
    }

    fn assert_no_route(routes: &[WitnessRoute], kind: WitnessKind) {
        assert!(
            routes.iter().all(|route| route.kind != kind),
            "unexpected route {kind:?}"
        );
    }
}
