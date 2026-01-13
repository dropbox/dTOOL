# Introduction

inky is a Rust-native terminal UI library with a React-like component model and flexbox
layout powered by Taffy. It targets high performance, low memory use, and ergonomic UI
composition for terminal applications.

## Goals

- Fast rendering with incremental diffing.
- A component tree API that feels familiar to React users.
- Predictable layout via flexbox.
- Low memory overhead suitable for long-running CLI tools.

## Stability

This guide calls out APIs as **stable** or **unstable** where relevant. Stable APIs are
intended to remain compatible within the current 0.x series. Unstable APIs may change
while the library iterates toward 1.0.
