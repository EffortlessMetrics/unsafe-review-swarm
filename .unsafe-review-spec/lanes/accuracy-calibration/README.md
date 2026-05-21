# Accuracy validation and calibration lane

This lane defines the proof system for "how often are ReviewCards right, useful,
and not overclaiming" under explicit claim scopes.

Pipeline:

1. Fixtures pin exact behavior.
2. Dogfood measures real-world exposure on selected corpora.
3. Labeled calibration establishes adjudicated denominators.
4. Calibration reports publish scoped metrics and known limits.
5. Claim promotion gates protect public wording.

The lane is intentionally advisory-first: no global precision/recall claims, no
memory-safety proof claims, and no default blocking policy claims.
