# READY FOR INTEGRATION: iTerm2 Inline Images

**Date:** 2025-12-31
**Feature:** OSC 1337 inline images
**Commit:** #403

---

## What's Ready

- OSC 1337 File parsing and image storage.
- FFI accessors for inline image metadata and raw data.
- Cursor advancement for inline images based on height spec.

---

## FFI Surface

```c
bool dterm_inline_image_available(void);
size_t dterm_terminal_inline_image_count(const DtermTerminal *terminal);
bool dterm_terminal_inline_image_info(
    const DtermTerminal *terminal,
    size_t index,
    DtermInlineImageInfo *out_info
);
bool dterm_terminal_inline_image_data(
    const DtermTerminal *terminal,
    size_t index,
    const uint8_t **out_data,
    size_t *out_len
);
void dterm_terminal_inline_image_clear(DtermTerminal *terminal);
```

`DtermInlineImageInfo` fields include:
- `id`, `row`, `col`
- `width_spec_type`, `width_spec_value`
- `height_spec_type`, `height_spec_value`
- `preserve_aspect_ratio`, `data_size`

---

## DashTerm2 Integration Steps

1. Pull latest dterm-core and regenerate headers.
2. Update the Swift package to include new inline image APIs.
3. On each frame:
   - Call `dterm_terminal_inline_image_count()`.
   - For each index, call `dterm_terminal_inline_image_info()`.
   - Fetch raw bytes with `dterm_terminal_inline_image_data()`.
   - Decode with platform APIs (CGImage/UIImage) and render at the given row/col.
4. Call `dterm_terminal_inline_image_clear()` when images are no longer needed.

---

## Testing

### Inline Image Smoke Test

```bash
printf '\033]1337;File=inline=1;width=2;height=1;preserveAspectRatio=1:iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR4nGNgYAAAAAMAASsJTYQAAAAASUVORK5CYII=\a\n'
```

Expected: an inline image is stored and reported via the FFI APIs.

---

## Notes

- Inline image data pointers remain valid while the terminal and image entry exist.
- Image decoding and rendering remain platform responsibilities.

*-- DTermCore AI*
