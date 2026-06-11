use crate::domain::{HazardKind, WitnessKind, WitnessRoute};

/// Validate that an owner-derived string is safe to splice as an argv token
/// into a witness command.
///
/// Accepts ASCII identifier characters (`[A-Za-z0-9_]`), Rust path separators
/// (`::`) and hyphens (`-`) so that both function names and crate/module names
/// are accepted.  Rejects any token that starts with `--` (flag-shaped attack
/// vector) or is empty.  All other bytes — spaces, shell metacharacters, lone
/// leading dashes — are rejected.
fn is_safe_test_filter(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Reject flag-shaped tokens: any token starting with "--" would be
    // interpreted as a CLI flag by cargo and must never be spliced into argv.
    if name.starts_with("--") {
        return false;
    }
    // Accept ASCII ident chars, Rust path separator ::, and hyphens.
    name.chars()
        .all(|ch| ch == '_' || ch == '-' || ch == ':' || ch.is_ascii_alphanumeric())
}

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
                command: owner
                    .filter(|name| is_safe_test_filter(name))
                    .map(|name| format!("cargo test {name}_loom -- --nocapture")),
                required: false,
            },
            WitnessRoute {
                kind: WitnessKind::Shuttle,
                reason: "Shuttle can explore scheduler interleavings for focused concurrency invariants".to_string(),
                command: owner
                    .filter(|name| is_safe_test_filter(name))
                    .map(|name| format!("cargo test {name}_shuttle -- --nocapture")),
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
                    |name| {
                        if is_safe_test_filter(name) {
                            format!("RUSTFLAGS='-Z sanitizer=address' cargo +nightly test {name}")
                        } else {
                            "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test".to_string()
                        }
                    },
                )),
                required: false,
            },
            WitnessRoute {
                kind: WitnessKind::CargoCareful,
                reason: "cargo-careful can cheaply exercise Rust-side FFI boundary assumptions when a focused test exists".to_string(),
                command: Some(owner.map_or_else(
                    || "cargo +nightly careful test".to_string(),
                    |name| {
                        if is_safe_test_filter(name) {
                            format!("cargo +nightly careful test {name}")
                        } else {
                            "cargo +nightly careful test".to_string()
                        }
                    },
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
            command: owner
                .filter(|name| is_safe_test_filter(name))
                .map(|name| format!("cargo +nightly miri test {name}")),
            required: false,
        }, WitnessRoute {
            kind: WitnessKind::CargoCareful,
            reason: "cargo-careful is a cheaper compatibility-oriented runtime check for several unsafe preconditions".to_string(),
            command: owner
                .filter(|name| is_safe_test_filter(name))
                .map(|name| format!("cargo +nightly careful test {name}")),
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

    #[test]
    fn is_safe_test_filter_accepts_normal_identifiers() {
        assert!(is_safe_test_filter("read_header"));
        assert!(is_safe_test_filter("my_function_name"));
        assert!(is_safe_test_filter("Foo123"));
    }

    #[test]
    fn is_safe_test_filter_accepts_rust_path_and_hyphenated_names() {
        assert!(is_safe_test_filter("module::fn_name"));
        assert!(is_safe_test_filter("my-crate"));
        assert!(is_safe_test_filter("some::deeply::nested::fn_name"));
    }

    #[test]
    fn is_safe_test_filter_rejects_flag_shaped_tokens() {
        assert!(!is_safe_test_filter("--config"));
        assert!(!is_safe_test_filter("--"));
        assert!(!is_safe_test_filter("--anything"));
    }

    #[test]
    fn is_safe_test_filter_rejects_empty_and_shell_metacharacters() {
        assert!(!is_safe_test_filter(""));
        assert!(!is_safe_test_filter("name with spaces"));
        assert!(!is_safe_test_filter("name;injection"));
        assert!(!is_safe_test_filter("name|pipe"));
        assert!(!is_safe_test_filter("name&ampersand"));
    }

    #[test]
    fn routes_for_returns_none_command_when_owner_fails_charset_check() {
        let bad_owner = "--config".to_string();
        let routes = routes_for(&[HazardKind::Alignment], Some(&bad_owner));

        // When the owner token fails the charset check, command must be None —
        // no argv token is fabricated from an invalid owner.
        for route in &routes {
            assert!(
                route.command.is_none(),
                "expected command: None for invalid owner; got {:?} on route {:?}",
                route.command,
                route.kind
            );
        }
    }

    #[test]
    fn ffi_sanitizer_fallback_with_invalid_owner_broadens_to_suite() {
        // When owner fails the charset check on an FFI hazard, the sanitizer
        // route must fall back to the whole-suite command (honest, not narrowed).
        let bad_owner = "--config".to_string();
        let routes = routes_for(
            &[HazardKind::FfiAbi, HazardKind::FfiOwnership],
            Some(&bad_owner),
        );

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
    }
}
