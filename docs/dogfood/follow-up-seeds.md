# Dogfood follow-up seed index

Status: experimental dogfood backlog index

This index turns dogfood observations into small PR seeds. It is not a task
tracker for every possible analyzer idea; each row must point back to a checked
dogfood report, name the corpus target, keep one primary triage label, and keep
the next PR slice narrow enough to review.

Statuses:

- `open`: ready for a narrow swarm PR.
- `done`: the linked report's follow-up has landed or is covered by a later
  verifier/refactor.
- `parked`: recorded for future pressure, but not enough evidence for an
  implementation PR.
- `superseded`: replaced by a newer seed or report.

## Seeds

| Seed ID | Status | Target | Family/surface | Primary label | Source report | Next PR slice | Notes |
|---|---|---|---|---|---|---|---|
| `dogfood-arrayvec-pr288-vec-set-len` | `done` | `arrayvec-pr288` | `vec_set_len` | `actionable` | [arrayvec Vec::set_len rerun](reports/2026-05-26-arrayvec-vec-set-len-rerun.md) | `analysis: keep vec_set_len initialized-range regression pressure` | Current follow-up is covered by the initialized-range applicability work; add a new fixture only for a future stale or wrong-target dogfood shape. |
| `dogfood-arrayvec-pr137-posture-delta` | `done` | `arrayvec-pr137` | `repo_posture` | `needs-doc` | [post-burst analyzer snapshot](reports/2026-05-26-post-burst.md) | `docs: keep dogfood posture movement actionability-centered` | Current reports record that raw card counts can look worse for soundness-oriented PRs; keep future outcome/posture docs focused on review movement rather than safety scores. |
| `dogfood-arrayvec-pr138-unsafe-function-wording` | `parked` | `arrayvec-pr138` | `unsafe_fn_call` | `actionable` | [arrayvec PR 138 UTF-8 follow-up](reports/2026-05-27-arrayvec-pr138-utf8-follow-up.md) | `analysis: use arrayvec-pr138 only for unsafe-function and raw-pointer wording pressure` | The target is useful for unsafe-function, raw-pointer, pointer-arithmetic, and Vec::set_len cards, but it should not broaden UTF-8 unchecked recognizers without a matching real card. |
| `dogfood-crossbeam-pr1226-atomic-pointer-state` | `done` | `crossbeam-pr1226` | `atomic_pointer_state` | `actionable` | [crossbeam atomic pointer rerun](reports/2026-05-26-crossbeam-atomic-pointer-rerun.md) | `analysis: keep atomic pointer-state fixture regression pressure` | Current follow-up is covered by the atomic pointer-state classification and route fixtures; add another fixture only for a new concrete missing shape. |
| `dogfood-memchr-capped-unknown-comment-plan` | `done` | `memchr-capped` | `comment_plan` | `noise` | [memchr unknown comment-plan follow-up](reports/2026-05-26-memchr-unknown-comment-plan.md) | `comment-plan: keep unknown-family cards out of inline candidates` | The broad unknown cards remain in artifacts but are excluded from planned inline comments. |
| `dogfood-mio-pr1388-ffi-route-wording` | `done` | `mio-pr1388` | `ffi` | `needs-route` | [mio FFI route wording](reports/2026-05-26-mio-ffi-route-wording.md) | `analysis: keep ffi boundary route wording human-review-heavy` | Current follow-up is covered by the FFI route wording verifier; do not imply Miri coverage. |
| `dogfood-arrayvec-pr138-utf8` | `parked` | `arrayvec-pr138` | `str_from_utf8_unchecked` | `needs-doc` | [arrayvec PR 138 UTF-8 follow-up](reports/2026-05-27-arrayvec-pr138-utf8-follow-up.md) | `docs: avoid citing arrayvec-pr138 as UTF-8 unchecked dogfood` | This target does not exercise `str::from_utf8_unchecked`; keep UTF-8 unchecked applicability fixture-backed until real dogfood appears. |
| `dogfood-hashbrown-pr667-nonnull` | `parked` | `hashbrown-pr667` | `nonnull_unchecked` | `actionable` | [hashbrown NonNull follow-up](reports/2026-05-27-hashbrown-nonnull-follow-up.md) | `analysis: add nonnull macro or provenance control only when dogfood exposes it` | Existing stale and wrong-pointer controls cover current pressure; future macro/cast/provenance shapes need separate evidence. |
| `dogfood-memchr-capped-target-feature` | `done` | `memchr-capped` | `target_feature` | `actionable` | [memchr target-feature posture](reports/2026-05-28-memchr-target-feature-posture.md) | `output: keep target_feature cards contract-backed and unwitnessed` | Preserve target-feature docs as contract evidence without turning them into availability, site-execution, or Miri evidence. |
| `dogfood-arrayvec-pr288-first-pr-projection` | `done` | `arrayvec-pr288` | `first_pr_projection` | `actionable` | [arrayvec first-pr projection smoke](reports/2026-05-28-arrayvec-first-pr-projection-smoke.md) | `output: rerun first-pr smoke after projection changes` | Use this target as regression pressure for first-pr, comment-plan, witness-plan, saved LSP, and agent-context projections. |
| `dogfood-arrayvec-pr288-self-new-capacity` | `done` | `arrayvec-pr288` | `vec_set_len` | `needs-fixture` | [arrayvec Self::new capacity control](reports/2026-05-29-arrayvec-self-new-capacity-control.md) | `analysis: pin self-new const-capacity set_len control` | Current follow-up is fixture `vec_set_len_self_new_const_cap_not_guard`; do not infer custom constructor capacity without visible same-receiver evidence. |

## Trust boundary

Dogfood follow-up seeds are static advisory review notes. They are not a proof
of memory safety, not UB-free status, not Miri-clean status, not site execution
evidence, not calibrated precision or recall, not witness adequacy, and not
policy readiness. A seed does not justify analyzer breadth until a narrow
fixture, verifier, or report-backed PR can preserve the advisory boundary.
