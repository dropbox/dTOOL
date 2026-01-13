# READY FOR INTEGRATION: Underline Styles

**Date:** 2025-12-31
**Feature:** Dotted and dashed underlines (SGR 4:4, SGR 4:5)
**Commit:** #402

---

## What's Ready

- SGR subparameter parsing supports 4:4 (dotted) and 4:5 (dashed).
- New underline styles are emitted in the GPU pipeline.
- Cell flags and style mapping updated for dotted/dashed underlines.

---

## Testing

### In DashTerm2

Run the following in a shell and confirm underline styles render correctly:

```bash
printf '\033[4:4mDotted\033[0m \033[4:5mDashed\033[0m\n'
```

---

## DashTerm2 Integration Steps

1. Pull latest dterm-core.
2. Rebuild the Rust library and update the Swift package.
3. Verify dotted and dashed underline rendering in the UI.

---

## Notes

- Existing underline styles (single/double/curly) are unchanged.

*-- DTermCore AI*
