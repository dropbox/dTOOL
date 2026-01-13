# Foot Terminal Emulator Analysis

**Author:** dTerm Research Team
**Date:** 2025-12-28
**Purpose:** Identify patterns for dTerm's low-latency terminal design

## Overview

Foot is a fast, lightweight, Wayland-native terminal emulator written in C. It achieves sub-millisecond rendering through careful architectural decisions, Wayland-specific optimizations, and efficient damage tracking.

**Key Source Files:**
- `/Users/ayates/dterm/research/foot/vt.c` (1133 lines) - VT parser state machine
- `/Users/ayates/dterm/research/foot/render.c` (5454 lines) - Rendering pipeline
- `/Users/ayates/dterm/research/foot/terminal.c` (4811 lines) - Terminal state management
- `/Users/ayates/dterm/research/foot/grid.c` (1676 lines) - Grid/scrollback management
- `/Users/ayates/dterm/research/foot/shm.c` - Shared memory buffer management
- `/Users/ayates/dterm/research/foot/server.c` - Daemon mode implementation
- `/Users/ayates/dterm/research/foot/fdm.c` - Event loop (epoll-based)

---

## 1. VT Parser Implementation

**File:** `/Users/ayates/dterm/research/foot/vt.c`

### State Machine Design

Foot implements the DEC ANSI parser based on the [vt100.net state machine](https://vt100.net/emu/dec_ansi_parser). The parser is a hand-written state machine with explicit state transitions.

```c
enum state {
    STATE_GROUND,
    STATE_ESCAPE,
    STATE_ESCAPE_INTERMEDIATE,
    STATE_CSI_ENTRY,
    STATE_CSI_PARAM,
    STATE_CSI_INTERMEDIATE,
    STATE_CSI_IGNORE,
    STATE_OSC_STRING,
    STATE_DCS_ENTRY,
    STATE_DCS_PARAM,
    STATE_DCS_INTERMEDIATE,
    STATE_DCS_IGNORE,
    STATE_DCS_PASSTHROUGH,
    STATE_SOS_PM_APC_STRING,
    STATE_UTF8_21,  // UTF-8 2-byte sequence
    STATE_UTF8_31,  // UTF-8 3-byte sequence 1/3
    STATE_UTF8_32,  // UTF-8 3-byte sequence 2/3
    STATE_UTF8_41,  // UTF-8 4-byte sequence states...
    STATE_UTF8_42,
    STATE_UTF8_43,
};
```

### Main Parse Loop

The core parsing function processes bytes one at a time with a switch on current state:

```c
void vt_from_slave(struct terminal *term, const uint8_t *data, size_t len)
{
    enum state current_state = term->vt.state;

    const uint8_t *p = data;
    for (size_t i = 0; i < len; i++, p++) {
        switch (current_state) {
        case STATE_GROUND:              current_state = state_ground_switch(term, *p); break;
        case STATE_ESCAPE:              current_state = state_escape_switch(term, *p); break;
        case STATE_CSI_ENTRY:           current_state = state_csi_entry_switch(term, *p); break;
        // ... other states
        case STATE_UTF8_21:             current_state = state_utf8_21_switch(term, *p); break;
        // ... UTF-8 continuation states
        }
        term->vt.state = current_state;
    }
}
```

### UTF-8 Decoding

UTF-8 is decoded inline within the state machine using dedicated states for multi-byte sequences:

```c
static void action_utf8_21(struct terminal *term, uint8_t c) {
    // wc = ((utf8[0] & 0x1f) << 6) | (utf8[1] & 0x3f)
    term->vt.utf8 = (c & 0x1f) << 6;
}

static void action_utf8_22(struct terminal *term, uint8_t c) {
    term->vt.utf8 |= c & 0x3f;
    action_utf8_print(term, term->vt.utf8);
}
```

### Key Observations

1. **No SIMD** - The parser is scalar, processing one byte at a time
2. **Inline UTF-8** - UTF-8 decoding integrated into state machine (avoids separate pass)
3. **Range-based switch** - Uses GCC extension `case 0x20 ... 0x7e:` for compact dispatch
4. **Minimal allocations** - Parameters stored in fixed-size arrays in terminal struct

### dTerm Implications

- Consider SIMD for bulk ASCII detection (scan for escape codes)
- State machine approach is proven and maintainable
- UTF-8 inline decoding avoids extra passes
- Rust enums + match would map well to this pattern

---

## 2. Rendering Pipeline

**Files:** `/Users/ayates/dterm/research/foot/render.c`, `/Users/ayates/dterm/research/foot/shm.c`

### Architecture

Foot uses **CPU rendering with Pixman** to shared memory buffers (Wayland SHM). No GPU acceleration.

```
PTY Input -> VT Parser -> Grid Update -> Damage Tracking -> Pixman Render -> SHM Buffer -> Wayland Compositor
```

### Delayed Rendering

Foot intentionally delays rendering to batch updates:

```c
// From terminal.c - fdm_ptmx()
uint64_t lower_ns = term->conf->tweak.delayed_render_lower_ns;
uint64_t upper_ns = term->conf->tweak.delayed_render_upper_ns;

if (lower_ns > 0 && upper_ns > 0) {
    // Short delay to batch rapid updates
    timerfd_settime(term->delayed_render_timer.lower_fd, 0,
        &(struct itimerspec){.it_value = {.tv_nsec = lower_ns}}, NULL);

    // Upper bound prevents indefinite delay
    if (!term->delayed_render_timer.is_armed) {
        timerfd_settime(term->delayed_render_timer.upper_fd, 0,
            &(struct itimerspec){.it_value = {.tv_nsec = upper_ns}}, NULL);
        term->delayed_render_timer.is_armed = true;
    }
}
```

### Worker Threads

Rendering can be parallelized across rows using worker threads:

```c
int render_worker_thread(void *_ctx)
{
    struct render_worker_context *ctx = _ctx;
    struct terminal *term = ctx->term;
    const int my_id = ctx->my_id;

    while (true) {
        sem_wait(start);

        while (!frame_done) {
            mtx_lock(lock);
            int row_no = tll_pop_front(term->render.workers.queue);
            mtx_unlock(lock);

            switch (row_no) {
            default:
                render_row(term, buf->pix[my_id], &buf->dirty[my_id],
                           row, row_no, cursor_col);
                break;
            case -1:  // Frame done signal
                frame_done = true;
                sem_post(done);
                break;
            case -2:  // Shutdown signal
                return 0;
            }
        }
    }
}
```

### Damage Tracking

Foot tracks damage at cell granularity using dirty bits:

```c
// In grid_render()
for (int r = 0; r < term->rows; r++) {
    struct row *row = grid_row_in_view(term->grid, r);

    if (!row->dirty)
        continue;

    row->dirty = false;
    // Queue row for rendering
}

// Cell-level dirty tracking
struct cell {
    // ...
    struct {
        // ...
        unsigned clean : 1;  // Cell needs re-rendering if 0
    } attrs;
};
```

### Buffer Reuse and Pre-applied Damage

Foot optimizes multi-buffering by copying unchanged regions:

```c
static void reapply_old_damage(struct terminal *term, struct buffer *new, struct buffer *old)
{
    if (new->age > 1) {
        memcpy(new->data, old->data, new->height * new->stride);
        return;
    }

    // Calculate dirty region, subtract from old damage
    pixman_region32_subtract(&dirty, &old->dirty[0], &dirty);
    pixman_image_set_clip_region32(new->pix[0], &dirty);

    pixman_image_composite32(
        PIXMAN_OP_SRC, old->pix[0], NULL, new->pix[0],
        0, 0, 0, 0, 0, 0, term->width, term->height);
}
```

### SHM Buffer Management

Foot uses Wayland shared memory with sophisticated buffer pooling:

```c
// From shm.c
struct buffer_pool {
    int fd;                    // memfd
    struct wl_shm_pool *wl_pool;
    void *real_mmapped;        // Address returned from mmap
    size_t mmap_size;
    size_t ref_count;
};

struct buffer_private {
    struct buffer public;
    bool busy;                 // Owned by compositor
    struct buffer_pool *pool;
    off_t offset;              // Offset into memfd
    // ...
};
```

**SHM Scrolling Optimization:** On 64-bit systems, foot can scroll the buffer in-place using `fallocate(FALLOC_FL_PUNCH_HOLE)`:

```c
// Scroll by adjusting offset in memfd, punch holes to free unused memory
if (fallocate(pool->fd, FALLOC_FL_PUNCH_HOLE | FALLOC_FL_KEEP_SIZE,
              trim_ofs, trim_len) < 0) {
    LOG_ERRNO("failed to trim SHM backing memory file");
}
```

### dTerm Implications

1. **CPU rendering viable** - Pixman is fast enough for terminals
2. **Damage tracking critical** - Cell-level dirty bits essential
3. **Batched rendering** - Delay rendering to batch rapid updates
4. **Worker thread pool** - Per-row parallelization effective
5. **Buffer management** - Complex but necessary for performance

---

## 3. Daemon Mode (footserver/footclient)

**File:** `/Users/ayates/dterm/research/foot/server.c`

### Architecture

```
footserver (daemon)
    |
    +-- Wayland connection (shared)
    +-- Font cache (shared)
    +-- Glyph cache (shared)
    |
    +-- Terminal Instance 1
    +-- Terminal Instance 2
    +-- ...

footclient ----[Unix Socket]----> footserver
```

### Server Structure

```c
struct server {
    struct config *conf;
    struct fdm *fdm;           // Event loop
    struct reaper *reaper;     // Process reaper
    struct wayland *wayl;      // Shared Wayland connection

    int fd;                    // Listen socket
    const char *sock_path;

    tll(struct client *) clients;
    tll(struct terminal_instance *) terminals;
};

struct client {
    struct server *server;
    int fd;
    struct terminal_instance *instance;
    // Buffer for receiving init data
};

struct terminal_instance {
    struct terminal *terminal;
    struct server *server;
    struct client *client;
    struct config *conf;       // Per-instance config overrides
};
```

### Communication Protocol

1. Client connects via Unix socket
2. Client sends initialization data (args, env, config overrides)
3. Server creates terminal instance
4. Client remains connected to receive exit code
5. Server sends exit code when terminal closes

```c
// Client sends total length first
uint32_t total_len;
ssize_t count = recv(fd, &total_len, sizeof(total_len), 0);

// Then receives all init data
count = recv(fd, &client->buffer.data[client->buffer.idx],
             client->buffer.left, 0);
```

### Benefits

1. **Shared resources** - Fonts, glyph cache shared across terminals
2. **Instant spawn** - No font loading latency for new windows
3. **Lower memory** - Shared Wayland connection and caches

### Tradeoffs

1. **Single point of failure** - Server crash kills all terminals
2. **I/O multiplexing** - All terminals share one event loop
3. **Complexity** - Additional IPC protocol

### dTerm Implications

- Daemon mode provides instant window spawn
- Critical for AI agents spawning many terminals
- Consider process isolation (separate renderers?)
- Font/glyph cache sharing most valuable

---

## 4. Memory Model - Grid Structure

**Files:** `/Users/ayates/dterm/research/foot/grid.h`, `/Users/ayates/dterm/research/foot/grid.c`

### Grid Layout

```c
struct grid {
    int num_rows;              // Power of 2 for fast modulo
    int num_cols;
    struct row **rows;         // Circular buffer
    int offset;                // Current screen start
    int view;                  // View position (for scrollback)
    struct cursor cursor;
    // ...
};

struct row {
    struct cell *cells;
    bool dirty;                // Row needs re-render
    struct row_data *extra;    // URI ranges, underline data
};

struct cell {
    char32_t wc;               // Unicode codepoint
    struct {
        uint8_t fg;            // Foreground color index
        uint8_t bg;            // Background color
        // Various attribute bits packed together
        unsigned bold : 1;
        unsigned italic : 1;
        unsigned underline : 1;
        unsigned clean : 1;    // Rendering dirty flag
        // ...
    } attrs;
};
```

### Circular Buffer Design

Grid uses power-of-2 sizing for fast modulo:

```c
static inline int grid_row_absolute(const struct grid *grid, int row_no)
{
    return (grid->offset + row_no) & (grid->num_rows - 1);
}

static inline struct row *grid_row(struct grid *grid, int row_no)
{
    int real_row = grid_row_absolute(grid, row_no);
    return grid->rows[real_row];
}
```

### Lazy Row Allocation

Rows are allocated on-demand:

```c
static inline struct row *
_grid_row_maybe_alloc(struct grid *grid, int row_no, bool alloc_if_null)
{
    int real_row = grid_row_absolute(grid, row_no);
    struct row *row = grid->rows[real_row];

    if (row == NULL && alloc_if_null) {
        row = grid_row_alloc(grid->num_cols, false);
        grid->rows[real_row] = row;
    }

    return row;
}
```

### dTerm Implications

1. **Power-of-2 sizing** - Fast modulo via bitmask
2. **Lazy allocation** - Don't allocate scrollback until needed
3. **Packed cell attrs** - Minimize memory per cell
4. **Dirty flags** - Essential for incremental rendering

---

## 5. Threading and I/O Model

**File:** `/Users/ayates/dterm/research/foot/fdm.c`

### Event Loop

Foot uses epoll-based event multiplexing:

```c
// From fdm.c (inferred from usage patterns)
// Main loop structure:
// 1. epoll_wait() for events
// 2. Dispatch to registered handlers
// 3. Handle PTY input -> parse -> damage
// 4. Handle timers -> render

// Example from terminal.c
bool fdm_ptmx(struct fdm *fdm, int fd, int events, void *data)
{
    struct terminal *term = data;

    const bool pollin = events & EPOLLIN;
    const bool hup = events & EPOLLHUP;

    uint8_t buf[24 * 1024];
    const size_t max_iterations = !hup ? 10 : SIZE_MAX;

    for (size_t i = 0; i < max_iterations && pollin; i++) {
        ssize_t count = read(term->ptmx, buf, sizeof(buf));
        if (count <= 0) break;

        vt_from_slave(term, buf, count);
    }

    // Schedule render
    render_refresh(term);
}
```

### Thread Model

```
Main Thread:
  - Event loop (epoll)
  - PTY I/O
  - VT parsing
  - Wayland protocol

Render Worker Threads (optional):
  - Per-row rendering
  - Synchronized via semaphores

No separate PTY read thread - all I/O in main event loop.
```

### Key Design Decisions

1. **Single main thread** - All I/O and Wayland in one thread
2. **Optional render workers** - Parallel row rendering
3. **Non-blocking I/O** - PTY set to O_NONBLOCK
4. **Batched reads** - Up to 24KB per iteration

### dTerm Implications

- Single event loop simplifies coordination
- Render parallelization provides speedup
- Non-blocking PTY essential
- Consider async I/O for multiple PTYs

---

## 6. Wayland Integration

**File:** `/Users/ayates/dterm/research/foot/wayland.c`

### Wayland-Native Advantages

1. **Direct rendering** - No X11 compositing overhead
2. **Frame callbacks** - Sync to compositor refresh
3. **Damage regions** - Only repaint changed areas
4. **Single-pixel buffer** - Efficient solid color fills

### Frame Callback Pattern

```c
static void frame_callback(void *data, struct wl_callback *wl_callback,
                           uint32_t callback_data)
{
    struct terminal *term = data;
    wl_callback_destroy(wl_callback);
    term->window->frame_callback = NULL;

    // Now safe to render next frame
    if (term->render.pending)
        grid_render(term);
}

// In grid_render():
term->window->frame_callback = wl_surface_frame(term->window->surface.surf);
wl_callback_add_listener(term->window->frame_callback, &frame_listener, term);
```

### Damage Reporting

```c
// Report damage to compositor
int box_count = 0;
pixman_box32_t *boxes = pixman_region32_rectangles(&damage, &box_count);

for (size_t i = 0; i < box_count; i++) {
    wl_surface_damage_buffer(
        term->window->surface.surf,
        boxes[i].x1, boxes[i].y1,
        boxes[i].x2 - boxes[i].x1, boxes[i].y2 - boxes[i].y1);
}
```

### SHM Format Selection

Foot supports multiple pixel formats:

```c
// From wayland.c
switch (format) {
case WL_SHM_FORMAT_XRGB2101010: wayl->shm_have_xrgb2101010 = true; break;
case WL_SHM_FORMAT_ARGB2101010: wayl->shm_have_argb2101010 = true; break;
// ... etc
}
```

### dTerm Implications

- Frame callbacks essential for smooth rendering
- Damage reporting reduces compositor work
- Must support multiple SHM formats
- Consider Vulkan for GPU rendering option

---

## 7. Configuration System

**File:** `/Users/ayates/dterm/research/foot/config.c`

### Configuration Structure

```c
// Default colors
static const uint32_t default_foreground = 0xffffff;
static const uint32_t default_background = 0x242424;

static const uint32_t default_color_table[256] = {
    // Regular 8 colors
    0x242424, 0xf62b5a, 0x47b413, 0xe3c401,
    0x24acd4, 0xf2affd, 0x13c299, 0xe6e6e6,
    // Bright 8 colors
    0x616161, 0xff4d51, 0x35d450, 0xe9e836,
    // ... 6x6x6 RGB cube, 24 grays
};
```

### INI-based Configuration

- File: `~/.config/foot/foot.ini`
- Hot-reloadable via SIGHUP
- Extensive key binding customization
- Per-terminal overrides via command line

### dTerm Implications

- INI/TOML simple and familiar
- Hot-reload important for users
- Consider YAML for complex nested config
- Key bindings need careful design

---

## 8. Performance Summary

### Why Foot is Fast

1. **CPU Rendering** - Pixman well-optimized, no GPU transfer overhead
2. **Damage Tracking** - Cell-level dirty bits minimize rendering
3. **Delayed Batching** - Coalesce rapid updates before render
4. **Worker Threads** - Parallel row rendering
5. **SHM Scrolling** - In-place scroll via fallocate
6. **Frame Callbacks** - Sync to compositor, no tearing
7. **Wayland Native** - No X11 compatibility overhead

### Performance Characteristics

- Sub-millisecond render for typical updates
- ~24KB PTY read buffer per iteration
- Up to 10 read iterations before yielding
- Configurable render delay (lower_ns, upper_ns)

---

## 9. Key Takeaways for dTerm

### Must Have
1. **Cell-level damage tracking** - Essential for incremental updates
2. **Frame callback sync** - Prevents tearing, reduces work
3. **Batched rendering** - Timer-based coalescing
4. **Circular scrollback** - Power-of-2 for fast modulo
5. **Lazy row allocation** - Don't allocate until needed

### Should Have
1. **Worker thread pool** - Parallel row rendering
2. **Daemon mode** - Instant spawn, shared caches
3. **SHM scrolling** - In-place scroll optimization
4. **Buffer reuse** - Copy unchanged regions

### Consider
1. **GPU rendering** - Vulkan/wgpu for complex scenes
2. **SIMD parsing** - Bulk ASCII detection
3. **Memory pooling** - Arena allocators for cells

### Avoid
1. **Over-engineering parser** - Simple state machine works
2. **GPU for everything** - CPU often faster for text
3. **Complex threading** - Single event loop is clean

---

## Appendix: Code Locations

| Feature | File | Function/Struct |
|---------|------|-----------------|
| VT Parser | vt.c | `vt_from_slave()` |
| State Machine | vt.c | `state_*_switch()` |
| Grid Structure | terminal.h | `struct grid` |
| Row Allocation | grid.c | `grid_row_alloc()` |
| Damage Tracking | render.c | `dirty_cursor()`, `row->dirty` |
| Render Loop | render.c | `grid_render()` |
| Worker Threads | render.c | `render_worker_thread()` |
| SHM Buffers | shm.c | `struct buffer_pool` |
| SHM Scroll | shm.c | `shm_scroll_forward()` |
| Frame Callback | render.c | `frame_callback()` |
| Daemon Mode | server.c | `struct server` |
| Event Loop | fdm.c | `fdm_*()` |
| PTY Handler | terminal.c | `fdm_ptmx()` |
| Config | config.c | `config_*()` |
