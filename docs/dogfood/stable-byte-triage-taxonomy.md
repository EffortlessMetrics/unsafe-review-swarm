# Bun stable-byte triage taxonomy

Status: experimental Bun dogfood vocabulary

This taxonomy labels stable-byte manual candidates and follow-up seeds for the
Bun burndown lane. It is a routing aid for candidate packets, fixtures, and
handoffs, not analyzer discovery and not a policy signal.

Use one or more labels when a seed needs to explain why it is the next useful
piece of work. Keep the label attached to the candidate identity and ledger
state rather than treating it as a separate truth.

## Labels

| Label | Use when | Typical next step | Do not infer |
|---|---|---|---|
| `observable` | A safe JS caller route is expected to produce wrong system behavior before the patch. | Ask for system-Bun red and patched-green evidence on the smallest PR aperture. | The witness proves memory safety, site execution for every sink, or UB-free status. |
| `non-observable` | The risk is nondiscriminating in system behavior, such as aliasing or lifetime shape without a visible crash. | Pair mutation pressure with a Miri or small model proof of the shape. | Source inspection alone makes the candidate sure UB. |
| `helper-gated` | A helper copy, pin, snapshot, or coercion boundary decides whether the route is already stable. | Park the seed with the exact helper check or unblock command. | The dependency is resolved, or the helper is safe without checking it. |
| `needs-node-parity` | Compatibility behavior must be compared with Node before choosing the patch or error order. | Add a Node comparison script or cite an existing comparison artifact. | Node parity is already established or policy-ready. |
| `needs-miri-model` | A nondiscriminating or aliasing route needs a focused Rust model before confidence can move. | Add or rerun the smallest model that captures the byte-lifetime shape. | The model proves the Bun site executed under Miri. |
| `needs-fixture` | The route should become a committed fixture or smoke input before analyzer or verifier changes. | Add one fixture/control seeded by the manual packet. | The fixture is runtime proof or calibrated recall evidence. |
| `needs-manual-candidate` | The scout observation has no canonical manual candidate artifact yet. | Write a `manual-candidate/v1` packet with file:line, proof mode, fix boundary, and stop line. | The analyzer discovered the route. |
| `needs-analyzer` | Manual workflow and fixture pressure are solid enough for one narrow advisory heuristic. | Open one detector PR for one family and one false-positive control. | Broad stable-byte analyzer expansion is justified. |
| `needs-ripr` | The seed is blocked on diff-first repository inventory or seam-cache behavior. | Record the exact ripr requirement or unblock command in [`ripr-bun-diff-first-requirements.md`](ripr-bun-diff-first-requirements.md). | The current unsafe-review artifact already has complete Bun-scale inventory. |
| `needs-tokmd` | The seed needs a packet preset or Markdown/JSON export shape for implementer handoff. | Add or update a tokmd-friendly packet export or preset contract. | The packet was executed, repaired, or posted. |

## Reporting Rules

- Prefer `observable` or `non-observable` before asking for proof work; this
  keeps system red/green and Miri/model work in the right lane.
- Use `helper-gated` only with the exact helper or dependency that blocks the
  next upstreamable PR.
- Use `needs-analyzer` only after a manual candidate and fixture/control can
  keep the heuristic advisory and narrow.
- Keep `needs-ripr` and `needs-tokmd` as tooling-interface labels; do not use
  them to claim candidate confidence.

## Trust Boundary

Stable-byte triage labels are advisory workflow metadata. They are not witness
execution, not a proof of memory-safety, not UB-free status, not Miri-clean
status, not site-execution proof, not calibrated precision or recall, and not
policy readiness.
