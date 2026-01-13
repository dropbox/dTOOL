/*
 * CDTermCore - C bindings for dterm-core
 *
 * This file is intentionally empty. The actual implementation is provided
 * by the dterm-core Rust library which must be linked when building.
 *
 * To use this package, you must:
 * 1. Build dterm-core as a static library (libdterm_core.a)
 * 2. Link against it in your Xcode project or build system
 *
 * Build dterm-core with:
 *   cargo build --release -p dterm-core --features ffi
 *
 * The static library will be in:
 *   target/release/libdterm_core.a (or target/<arch>/release/)
 */

// Placeholder to satisfy Swift Package Manager's requirement for source files
