# mixed_source_roles fixture (#2227)

Mixed repository for source-role-aware surfacing work. Every file holds an
intentional unsafe seam; complete inventory retains all of them. Role
classification (step 2) changes the selected reviewer view, never evidence
or detected counts.

| File | Intended role | Seam |
|---|---|---|
| `src/lib.rs` | production | raw-pointer read + `NonNull::new_unchecked` |
| `src/tested.rs` | test-only (`#[cfg(test)]` in `src/`) | unsafe test helper |
| `src/generated.rs` | generated production (`@generated`) | `get_unchecked` helper |
| `shared/span.rs` | shared/ambiguous (`#[path]`-included) | pointer arithmetic |
| `tests/fixtures/byte_input.rs` | fixture input | unchecked read |
| `examples/demo.rs` | example | raw-pointer write |

Current inventory: 7 cards across these 6 files (see
`mixed_source_roles_inventory_retains_every_seam`). Generated code remains
production review work; the `#[cfg(test)]` helper remains visible; the
shared file is inventoried at its real location.
