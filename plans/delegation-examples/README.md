# Delegation examples for #1927

Advisory corpus proving the bounded delegation contracts from #1924 / #1926
are understandable and context-bounded. No runtime, scheduler, or required metric.

## Bound

`opencode.json` sets `subagent_depth: 1` — the root may delegate one level,
but a subagent may not delegate further. Exceeding the bound is an overflow:
the parent must summarize with `selected / omitted / total` and stable overflow
references instead of copying the full log.

## Fixtures

- `valid-overflow.toml` — large search bounded: 3 of 43 findings inlined,
  remainder via `overflow.refs` artifact reference. Summary under budget,
  exact `issue`, `work_spec`, `base_sha` identities preserved.
- `invalid-overflow-missing-refs.toml` — same shape but missing overflow counts /
  refs for a large result, so it fails validation.
- `single-agent-control.toml` — deliberately narrow one-file docs fix
  (`docs/README.md` typo) kept single-agent. Delegation would be wasteful:
  cost of briefing, isolation, and synthesis exceeds the one-line change.

## Overflow contract

- Reference durable identities (issue, work_spec, file, head, command, receipt)
  rather than copying global doctrine.
- Root defaults remain bounded; complete evidence is retrievable only via
  stable artifact refs.
- Token / byte thresholds stay advisory until pilots measure useful limits.

## Claim boundary

These examples prove the packet pair is understandable and context-bounded.
They do not prove subagents improve throughput or correctness.
