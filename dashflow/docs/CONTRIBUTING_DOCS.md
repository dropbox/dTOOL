# Documentation Standards

**Last Updated:** 2026-01-03 (Worker #2421 - Fix stale CI reference)

Guidelines for maintaining consistent documentation across DashFlow.

## README Standards

### Crate READMEs

Every crate under `crates/` must have a README.md that:

1. **Title matches crate name** - `# dashflow-xyz` exactly
2. **Description matches Cargo.toml** - Use the same description from `Cargo.toml`
3. **Version is consistent** - Reference version `1.11` (matches workspace)
4. **Links are relative and correct** - Use `../../README.md` for main repo

**Template:** `templates/CRATE_README.md`

### Example App READMEs

Every example app under `examples/apps/` must have a README.md that:

1. **Describes its specific purpose** - Not a copy of main README
2. **Includes running instructions** - How to build and execute
3. **Lists configuration** - Environment variables and settings
4. **Links to docs/EXAMPLE_APPS.md** - For cross-reference

**Template:** `templates/EXAMPLE_APP_README.md`

### Validation

> **Note:** DashFlow uses internal Dropbox CI, not GitHub Actions. The `.github/` directory does not exist in this repository. Run validation locally before committing.

READMEs are validated using:
- `scripts/validate_readmes.py` - Checks structure, versions, titles

To validate locally:
```bash
python3 scripts/validate_readmes.py
```

## Code Examples in Documentation

When including code examples in markdown:

1. **Use language tags** - ` ```rust ` not just ` ``` `
2. **Include imports** - Show what to `use`
3. **Test examples** - Examples should compile (when possible)
4. **Show complete patterns** - Don't leave readers guessing

## Link Guidelines

1. **Relative paths from file location** - Not from repo root
2. **From docs/, use ../** - For crates, CLAUDE.md, etc.
3. **Verify links exist** - Use `scripts/check_docs.sh`

### Common Patterns

| From | To | Path |
|------|----|------|
| docs/*.md | CLAUDE.md | `../CLAUDE.md` |
| docs/*.md | crates/X/README.md | `../crates/X/README.md` |
| docs/*.md | other docs | `OTHER.md` (same directory) |
| crates/X/README.md | Main README | `../../README.md` |

## Version References

Always use version `1.11` for dashflow crates in documentation:
```toml
[dependencies]
dashflow = "1.11"
dashflow-openai = "1.11"
```

Exception: Non-dashflow dependencies use their own versions.

## Do NOT

1. **Copy main README to subdirectories** - Each README is unique
2. **Use absolute paths** - Always relative
3. **Reference non-existent files** - Verify links
4. **Write outdated versions** - Keep aligned with workspace

## Automation

- `scripts/validate_readmes.py` - Validates crate READMEs
- `scripts/check_docs.sh` - Validates documentation links
- `scripts/batch_update_readmes.sh` - Batch update crate READMEs
