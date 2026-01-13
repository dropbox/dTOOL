# Kani Harnesses (DashFlow)

This directory holds Kani proof harnesses for the `dashflow` crate.

## Current Harnesses

### StateGraph (KANI-002)

| Harness | Verifies |
|---------|----------|
| `proof_state_graph_new_no_panic` | Creating empty graph doesn't panic |
| `proof_state_graph_set_entry_point_empty` | Empty string entry point accepted |
| `proof_state_graph_set_entry_point_char` | Single char entry point works |
| `proof_state_graph_clone_equivalent` | Clone produces equivalent graph |
| `proof_state_graph_strict_mode` | Strict mode toggle doesn't panic |

## Run Harnesses

```bash
# Run a specific harness
cargo kani -p dashflow --harness proof_state_graph_new_no_panic

# Run all harnesses
cargo kani -p dashflow
```

## Known Limitations

**macOS CCRandomGenerateBytes**: StateGraph uses `uuid` crate which calls macOS
`CCRandomGenerateBytes` for random generation. Kani doesn't support this FFI call.
Harnesses that trigger UUID generation will fail with:

```
call to foreign "C" function `CCRandomGenerateBytes` is not currently supported
```

Workaround: Write harnesses that avoid code paths using random generation, or
contribute stubs to Kani (https://github.com/model-checking/kani/issues/2423).

## Guidelines

- Harnesses should be deterministic and bounded (small state spaces)
- Prefer verifying properties like "no panics", overflow safety, and determinism
- Keep harnesses narrowly scoped to the unit under proof (avoid full system setups)
- Use `kani::any()` for symbolic inputs with `kani::assume()` constraints

See `docs/kani/README.md` for toolchain setup.
