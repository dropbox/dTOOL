//! Cloudflare Workers AI integration for `DashFlow` Rust
//!
//! This crate provides Cloudflare Workers AI implementations for `DashFlow` Rust.
//!
//! # Features
//! - `ChatCloudflare`: Edge inference with 50+ models
//! - Streaming support for real-time responses
//! - Low-latency edge deployment
//! - Configurable sampling parameters

pub mod chat_models;

pub use chat_models::ChatCloudflare;
