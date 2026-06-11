# Public product surfaces lane closeout

Status: open

## Required evidence before closeout

- `cargo run --locked -p xtask -- check-public-surfaces`
- `cargo package -p unsafe-review --list`
- `cargo run --locked -p unsafe-review -- --help`
- `cargo run --locked -p xtask -- check-pr`
- `cargo run --locked -p xtask -- source-divergence`
- `git diff --check`

## Trust-boundary reminder

This lane must not claim memory safety proof, UB freedom, Miri cleanliness,
default witness execution, or default blocking policy.
