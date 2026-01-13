# Fuzz Corpus Policy

`cargo fuzz` generates large corpora and artifacts during runs. We do **not**
commit those generated files.

## What belongs in git

- A small, curated set of human-readable seed inputs per fuzz target.
- Files should use stable, descriptive names and an extension like `.txt` or
  `.bin` (so they are explicitly tracked by our `.gitignore` exceptions).

## What does NOT belong in git

- Auto-generated corpus files (typically 40-hex filenames).
- Crash artifacts (`fuzz/artifacts/`) and coverage output (`fuzz/coverage/`).

## If you find a crash

1. Minimize it with `cargo fuzz tmin <target> <path-to-crash>`.
2. Copy the minimized repro into `fuzz/corpus/<target>/` with a descriptive
   filename (e.g. `repro_<issue>.txt`).
3. Keep it small and explain the context in the PR/commit message.
