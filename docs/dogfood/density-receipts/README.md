# Dogfood density receipts

These checked-in receipts pin presentation-density observations to a named
dogfood target and exact commit. The inventory hash is the SHA-256 of the
sorted card IDs, one per line. `check-dogfood` verifies target identity,
unchanged raw-card inventory, summary arithmetic, and comment-plan accounting.

They are reviewability receipts only. They do not prove analyzer accuracy,
target-feature availability, witness or site execution, memory safety, UB-free
status, or Miri-clean status.

- [`memchr-target-feature.toml`](memchr-target-feature.toml) records the issue
  #1894 before/after target-feature summary and comment-plan density at the
  pinned `memchr-capped` input with an explicit 2,000-card scan cap.
