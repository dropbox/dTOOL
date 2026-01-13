// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use crate::errors::{Error, Result};
use crate::{CompressionType, DashStreamMessage, CURRENT_SCHEMA_VERSION};
use std::sync::LazyLock;
use prometheus::Counter;
use prost::Message;
use std::cell::RefCell;
use tracing::warn;

// ============================================================================
// Buffer Pool for Reduced Allocations in Hot Paths
// ============================================================================

// Thread-local buffer pool for encoding operations.
// Reduces heap allocations by reusing buffers across encode calls.
//
// SAFETY (M-194): This thread_local! RefCell pattern is safe in async contexts because:
// 1. All borrows are confined within synchronous `.with()` closures
// 2. No `.await` points exist between borrow and release
// 3. Async task migration only occurs at `.await` points
// 4. Each thread maintains its own isolated RefCell instance
// 5. acquire_buffer() and release_buffer() complete synchronously within `.with()`
//
// This is the idiomatic pattern for thread-local caching with async code.
thread_local! {
    /// Pool of reusable buffers for encoding (one per thread, no contention)
    static ENCODE_BUFFER_POOL: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
}

/// Default buffer capacity for pooled buffers (4KB - covers most messages)
const DEFAULT_BUFFER_CAPACITY: usize = 4096;

/// Maximum buffers to keep in pool per thread (prevents unbounded growth)
const MAX_POOL_SIZE: usize = 8;

// Prometheus metrics for compression behavior (M-624: Use centralized constants)
use crate::metrics_constants::METRIC_COMPRESSION_FAILURES_TOTAL;

static COMPRESSION_FAILURES_TOTAL: LazyLock<Counter> = LazyLock::new(|| {
    crate::metrics_utils::counter(
        METRIC_COMPRESSION_FAILURES_TOTAL,
        "Total number of message compression failures (fell back to uncompressed)",
    )
});

/// Best-effort update of protobuf header compression to match transport framing.
fn set_message_compression(message: &mut DashStreamMessage, compression: CompressionType) {
    let Some(inner) = message.message.as_mut() else {
        return;
    };

    match inner {
        crate::dash_stream_message::Message::Event(e) => {
            if let Some(h) = e.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::TokenChunk(t) => {
            if let Some(h) = t.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::StateDiff(s) => {
            if let Some(h) = s.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::ToolExecution(te) => {
            if let Some(h) = te.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::Checkpoint(c) => {
            if let Some(h) = c.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::Metrics(m) => {
            if let Some(h) = m.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::Error(err) => {
            if let Some(h) = err.header.as_mut() {
                h.compression = compression as i32;
            }
        }
        crate::dash_stream_message::Message::EventBatch(batch) => {
            if let Some(h) = batch.header.as_mut() {
                h.compression = compression as i32;
            }
            for event in &mut batch.events {
                if let Some(h) = event.header.as_mut() {
                    h.compression = compression as i32;
                }
            }
        }
        crate::dash_stream_message::Message::ExecutionTrace(trace) => {
            if let Some(h) = trace.header.as_mut() {
                h.compression = compression as i32;
            }
        }
    }
}

/// Acquire a buffer from the thread-local pool (or create a new one)
#[inline]
fn acquire_buffer() -> Vec<u8> {
    ENCODE_BUFFER_POOL.with(|pool| {
        pool.borrow_mut()
            .pop()
            .unwrap_or_else(|| Vec::with_capacity(DEFAULT_BUFFER_CAPACITY))
    })
}

/// Return a buffer to the thread-local pool for reuse
#[inline]
fn release_buffer(mut buf: Vec<u8>) {
    buf.clear();
    ENCODE_BUFFER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        if pool.len() < MAX_POOL_SIZE {
            pool.push(buf);
        }
        // If pool is full, buffer is dropped (prevents unbounded memory)
    });
}

/// Schema compatibility policy for validation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchemaCompatibility {
    /// Require exact version match (safest, recommended for production)
    #[default]
    Exact,
    /// Accept newer schemas (forward compatible)
    ForwardCompatible,
    /// Accept older schemas (backward compatible)
    BackwardCompatible,
}

/// Encode a DashStreamMessage to bytes
///
/// # Arguments
///
/// * `message` - The message to encode
///
/// # Returns
///
/// A vector of bytes containing the protobuf-encoded message
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::{DashStreamMessage, Event};
/// use dashflow_streaming::codec::encode_message;
///
/// let message = DashStreamMessage {
///     message: Some(dashflow_streaming::dash_stream_message::Message::Event(Event::default())),
/// };
/// let bytes = encode_message(&message).unwrap();
/// ```
pub fn encode_message(message: &DashStreamMessage) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(message.encoded_len());
    message.encode(&mut buf)?;
    Ok(buf)
}

/// Encode a DashStreamMessage using a pooled buffer (hot path optimization)
///
/// This function uses thread-local buffer pooling to reduce heap allocations
/// when encoding many messages in sequence (e.g., streaming telemetry).
///
/// # Arguments
///
/// * `message` - The message to encode
///
/// # Returns
///
/// A vector of bytes containing the protobuf-encoded message.
/// The returned Vec owns its data (not borrowed from pool).
///
/// # Performance
///
/// - First call: allocates new buffer (4KB default)
/// - Subsequent calls: reuses pooled buffers, avoiding allocations
/// - Thread-safe: each thread has its own pool (no contention)
///
/// # Limitation (M-520)
///
/// **Note:** This function clones the buffer contents before returning, which still
/// allocates memory for each call. The pooling only helps avoid allocations during
/// the encoding phase (buffer resizing). For true zero-copy performance, use
/// [`encode_message_into`] instead, which encodes directly into a caller-provided buffer.
///
/// Prefer `encode_message_into` when:
/// - You control the buffer lifecycle
/// - You need to minimize allocations
/// - You're encoding many messages in a tight loop
///
/// Use this function when:
/// - You need a simple API that returns owned data
/// - The clone overhead is acceptable for your use case
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::{DashStreamMessage, Event};
/// use dashflow_streaming::codec::encode_message_pooled;
///
/// // Hot path - reuses buffers during encoding, but clones on return
/// for _ in 0..1000 {
///     let message = DashStreamMessage {
///         message: Some(dashflow_streaming::dash_stream_message::Message::Event(Event::default())),
///     };
///     let bytes = encode_message_pooled(&message).unwrap();
///     // Process bytes...
/// }
/// ```
pub fn encode_message_pooled(message: &DashStreamMessage) -> Result<Vec<u8>> {
    let encoded_len = message.encoded_len();
    let mut buf = acquire_buffer();

    // Ensure buffer has enough capacity
    if buf.capacity() < encoded_len {
        buf.reserve(encoded_len - buf.capacity());
    }

    message.encode(&mut buf)?;

    // Clone the data to return (buffer goes back to pool)
    let result = buf.clone();
    release_buffer(buf);

    Ok(result)
}

/// Encode a DashStreamMessage into a provided buffer (zero-copy hot path)
///
/// This is the most efficient encoding method when you can reuse the same
/// buffer across multiple encode operations.
///
/// # Arguments
///
/// * `message` - The message to encode
/// * `buf` - The buffer to encode into (will be cleared first)
///
/// # Returns
///
/// The number of bytes written to the buffer
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::{DashStreamMessage, Event};
/// use dashflow_streaming::codec::encode_message_into;
///
/// let mut buf = Vec::with_capacity(4096);
/// for _ in 0..1000 {
///     let message = DashStreamMessage {
///         message: Some(dashflow_streaming::dash_stream_message::Message::Event(Event::default())),
///     };
///     let len = encode_message_into(&message, &mut buf).unwrap();
///     // Use buf[..len]...
/// }
/// ```
pub fn encode_message_into(message: &DashStreamMessage, buf: &mut Vec<u8>) -> Result<usize> {
    buf.clear();
    let encoded_len = message.encoded_len();

    // Ensure buffer has enough capacity
    if buf.capacity() < encoded_len {
        buf.reserve(encoded_len - buf.capacity());
    }

    message.encode(buf)?;
    Ok(buf.len())
}

/// Decode a DashStreamMessage from bytes
///
/// # Arguments
///
/// * `bytes` - The bytes to decode
///
/// # Returns
///
/// The decoded DashStreamMessage
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::DashStreamMessage;
/// use dashflow_streaming::codec::{encode_message, decode_message};
///
/// # let message = DashStreamMessage::default();
/// let bytes = encode_message(&message).unwrap();
/// let decoded = decode_message(&bytes).unwrap();
/// ```
pub fn decode_message(bytes: &[u8]) -> Result<DashStreamMessage> {
    DashStreamMessage::decode(bytes).map_err(Error::ProtobufDecode)
}

/// Encode a DashStreamMessage to bytes with optional compression
///
/// # Arguments
///
/// * `message` - The message to encode
/// * `compress` - Whether to compress the encoded message
///
/// # Returns
///
/// A tuple of (bytes, is_compressed)
///
/// # Format
///
/// The encoded bytes include a 1-byte header:
/// - 0x00: Uncompressed protobuf message
/// - 0x01: Zstd-compressed protobuf message
///
/// This allows the consumer to know whether to decompress without trial-and-error.
pub fn encode_message_with_compression(
    message: &DashStreamMessage,
    compress: bool,
) -> Result<(Vec<u8>, bool)> {
    encode_message_with_compression_config(
        message,
        compress,
        DEFAULT_COMPRESSION_THRESHOLD,
        DEFAULT_COMPRESSION_LEVEL,
    )
}

/// Default compression threshold in bytes.
/// Messages smaller than this are not compressed.
pub const DEFAULT_COMPRESSION_THRESHOLD: usize = 512;

/// Default zstd compression level (1-22, higher = better ratio but slower).
/// Level 3 is a good balance for streaming telemetry.
pub const DEFAULT_COMPRESSION_LEVEL: i32 = 3;

/// Encode a message with configurable compression settings.
///
/// # Arguments
///
/// * `message` - The message to encode
/// * `compress` - Whether to attempt compression
/// * `threshold` - Minimum size in bytes for compression (smaller messages skip compression)
/// * `level` - Zstd compression level (1-22, higher = smaller output but slower)
///
/// # Returns
///
/// A tuple of (encoded bytes with header, was_compressed)
///
/// # Compression Level Guide
///
/// - Level 1-3: Fast compression, good for high-throughput telemetry
/// - Level 4-6: Balanced compression, good for most use cases
/// - Level 7+: High compression ratio, CPU intensive
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::{DashStreamMessage, Event};
/// use dashflow_streaming::codec::encode_message_with_compression_config;
///
/// let message = DashStreamMessage {
///     message: Some(dashflow_streaming::dash_stream_message::Message::Event(Event::default())),
/// };
/// // Use larger threshold (1KB) and higher compression for batch processing
/// let (bytes, compressed) = encode_message_with_compression_config(&message, true, 1024, 6).unwrap();
/// ```
pub fn encode_message_with_compression_config(
    message: &DashStreamMessage,
    compress: bool,
    threshold: usize,
    level: i32,
) -> Result<(Vec<u8>, bool)> {
    let encoded = encode_message(message)?;

    if compress && encoded.len() > threshold {
        // Only compress if message is larger than threshold
        match crate::compression::compress_zstd(&encoded, level) {
            Ok(compressed) => {
                // Only use compressed version if it's actually smaller
                if compressed.len() < encoded.len() {
                    // Clone message to set protobuf header compression to match framing.
                    let mut msg_with_flag = message.clone();
                    set_message_compression(&mut msg_with_flag, CompressionType::CompressionZstd);

                    let encoded_with_flag = encode_message(&msg_with_flag)?;

                    match crate::compression::compress_zstd(&encoded_with_flag, level) {
                        Ok(compressed_with_flag)
                            if compressed_with_flag.len() < encoded_with_flag.len() =>
                        {
                            let mut payload = vec![HEADER_COMPRESSED_ZSTD];
                            payload.extend(compressed_with_flag);
                            return Ok((payload, true));
                        }
                        Ok(_) => {
                            // Compression no longer beneficial after flag update; fall back.
                        }
                        Err(e) => {
                            COMPRESSION_FAILURES_TOTAL.inc();
                            warn!(
                                error = %e,
                                encoded_len = encoded_with_flag.len(),
                                "Zstd compression failed after flag update, falling back to uncompressed"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                COMPRESSION_FAILURES_TOTAL.inc();
                warn!(
                    error = %e,
                    encoded_len = encoded.len(),
                    "Zstd compression failed, falling back to uncompressed"
                );
            }
        }
    }

    // Prepend compression header (0x00 = uncompressed)
    let mut payload = vec![HEADER_UNCOMPRESSED];
    payload.extend(encoded);
    Ok((payload, false))
}

/// Default maximum payload size (10 MB) to prevent OOM/DoS attacks
pub const DEFAULT_MAX_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;

/// Compression header byte indicating uncompressed payload
pub const HEADER_UNCOMPRESSED: u8 = 0x00;
/// Compression header byte indicating zstd-compressed payload
pub const HEADER_COMPRESSED_ZSTD: u8 = 0x01;

/// Decode a DashStreamMessage from bytes with automatic decompression
///
/// # Deprecated
///
/// This function accepts legacy messages without headers, which is a security risk.
/// Use [`decode_message_strict`] instead for untrusted input.
///
/// # Arguments
///
/// * `bytes` - The bytes to decode
/// * `_is_compressed` - Deprecated parameter (kept for API compatibility, but ignored)
///
/// # Returns
///
/// The decoded DashStreamMessage
///
/// # Format
///
/// This function automatically detects compression based on the first byte:
/// - 0x00: Uncompressed protobuf message
/// - 0x01: Zstd-compressed protobuf message
/// - No header: Legacy uncompressed message (for backward compatibility)
///
/// # Security
///
/// This function enforces a maximum payload size to prevent OOM/DoS attacks.
/// Use `decode_message_with_decompression_and_limit` to specify a custom limit.
#[deprecated(
    since = "1.1.0",
    note = "Use decode_message_strict() for untrusted input. This function accepts legacy messages without headers which is a security risk."
)]
#[allow(deprecated)]
pub fn decode_message_with_decompression(
    bytes: &[u8],
    _is_compressed: bool,
) -> Result<DashStreamMessage> {
    decode_message_with_decompression_and_limit(bytes, DEFAULT_MAX_PAYLOAD_SIZE)
}

/// Decode a DashStreamMessage with a custom payload size limit
///
/// # Deprecated
///
/// This function accepts legacy messages without headers, which is a security risk.
/// Use [`decode_message_strict`] instead for untrusted input.
///
/// # Arguments
///
/// * `bytes` - The bytes to decode
/// * `max_size` - Maximum allowed payload size in bytes
///
/// # Returns
///
/// The decoded DashStreamMessage, or an error if payload exceeds max_size
#[deprecated(
    since = "1.1.0",
    note = "Use decode_message_strict() for untrusted input. This function accepts legacy messages without headers which is a security risk."
)]
pub fn decode_message_with_decompression_and_limit(
    bytes: &[u8],
    max_size: usize,
) -> Result<DashStreamMessage> {
    if bytes.is_empty() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty message payload",
        )));
    }

    // Security: Check payload size before any processing to prevent OOM/DoS.
    // Messages using framing include a 1-byte header; allow that overhead.
    let has_header = matches!(bytes[0], HEADER_UNCOMPRESSED | HEADER_COMPRESSED_ZSTD);
    let framed_max = if has_header {
        max_size.saturating_add(1)
    } else {
        max_size
    };
    if bytes.len() > framed_max {
        return Err(Error::InvalidFormat(format!(
            "Payload size {} bytes exceeds maximum {} bytes",
            bytes.len(),
            framed_max
        )));
    }

    // Check first byte for compression header
    match bytes[0] {
        HEADER_UNCOMPRESSED => {
            // Uncompressed message - size already checked above
            decode_message(&bytes[1..])
        }
        HEADER_COMPRESSED_ZSTD => {
            // Compressed message - use max_size for decompression limit
            let decompressed =
                crate::compression::decompress_zstd_with_limit(&bytes[1..], max_size)?;
            decode_message(&decompressed)
        }
        first_byte => {
            // No header byte - assume legacy uncompressed message
            // Security: Size already checked above
            // Warning: This path exists for backward compatibility only.
            // For untrusted input, use decode_message_strict() instead.
            tracing::debug!(
                first_byte = format!("0x{:02X}", first_byte),
                "Decoding message without compression header (legacy format)"
            );
            decode_message(bytes)
        }
    }
}

/// Decode a DashStreamMessage with strict header validation (security mode)
///
/// Unlike `decode_message_with_decompression_and_limit`, this function **rejects**
/// messages without a valid compression header byte (0x00 or 0x01). This prevents
/// attackers from sending malformed payloads that could cause deserialization panics.
///
/// # Arguments
///
/// * `bytes` - The bytes to decode
/// * `max_size` - Maximum allowed payload size in bytes
///
/// # Returns
///
/// The decoded DashStreamMessage, or an error if:
/// - The message is empty
/// - The payload exceeds max_size
/// - The first byte is not a valid header (0x00 or 0x01)
/// - Decompression or deserialization fails
///
/// # Security
///
/// Use this function when processing untrusted input. The legacy fallback mode
/// in `decode_message_with_decompression_and_limit` exists for backward compatibility
/// but should not be used with untrusted data.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::codec::decode_message_strict;
///
/// let result = decode_message_strict(&[0x00, /* protobuf bytes */], 1024 * 1024);
/// // Returns error for unknown header bytes
/// let invalid = decode_message_strict(&[0xFF, 0x00, 0x00], 1024);
/// assert!(invalid.is_err());
/// ```
pub fn decode_message_strict(bytes: &[u8], max_size: usize) -> Result<DashStreamMessage> {
    if bytes.is_empty() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty message payload",
        )));
    }

    // Security: Check payload size before any processing to prevent OOM/DoS.
    // Strict mode requires framing header; allow 1-byte overhead.
    let framed_max = max_size.saturating_add(1);
    if bytes.len() > framed_max {
        return Err(Error::InvalidFormat(format!(
            "Payload size {} bytes exceeds maximum {} bytes",
            bytes.len(),
            framed_max
        )));
    }

    // Strict mode: Only accept known header bytes
    match bytes[0] {
        HEADER_UNCOMPRESSED => {
            // Uncompressed message
            decode_message(&bytes[1..])
        }
        HEADER_COMPRESSED_ZSTD => {
            // Compressed message - use max_size for decompression limit
            let decompressed =
                crate::compression::decompress_zstd_with_limit(&bytes[1..], max_size)?;
            decode_message(&decompressed)
        }
        invalid_byte => {
            // Security: Reject unknown header bytes instead of assuming legacy format
            Err(Error::InvalidFormat(format!(
                "Invalid compression header byte: 0x{:02X}. Expected 0x00 (uncompressed) or 0x01 (zstd). \
                 Use decode_message_compatible for legacy format support.",
                invalid_byte
            )))
        }
    }
}

/// Decode a DashStreamMessage, accepting strict framed messages and a limited legacy format.
///
/// This helper is intended for operators migrating older topics. It accepts:
/// - Framed messages with a 1-byte header (0x00 / 0x01) via [`decode_message_strict`]
/// - Legacy unframed protobuf messages (best-effort), optionally supporting zstd payloads
///   when the input looks like a zstd frame.
///
/// For untrusted input, prefer [`decode_message_strict`] and avoid legacy decoding.
pub fn decode_message_compatible(bytes: &[u8], max_size: usize) -> Result<DashStreamMessage> {
    if bytes.is_empty() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty message payload",
        )));
    }

    let has_header = matches!(bytes[0], HEADER_UNCOMPRESSED | HEADER_COMPRESSED_ZSTD);
    let max_allowed = if has_header {
        max_size.saturating_add(1)
    } else {
        max_size
    };
    if bytes.len() > max_allowed {
        return Err(Error::InvalidFormat(format!(
            "Payload size {} bytes exceeds maximum {} bytes",
            bytes.len(),
            max_allowed
        )));
    }

    if has_header {
        return decode_message_strict(bytes, max_size);
    }

    // Legacy unframed payloads: attempt a safe zstd heuristic, otherwise treat as raw protobuf.
    const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD]; // 0xFD2FB528
    if bytes.len() >= ZSTD_MAGIC.len() && bytes[..ZSTD_MAGIC.len()] == ZSTD_MAGIC {
        let decompressed = crate::compression::decompress_zstd_with_limit(bytes, max_size)?;
        return decode_message(&decompressed);
    }

    decode_message(bytes)
}

/// Validates schema version compatibility
///
/// # Arguments
///
/// * `message_version` - The schema version from the message
/// * `policy` - The compatibility policy to apply
///
/// # Returns
///
/// `Ok(())` if version is compatible, `Err` otherwise
pub fn validate_schema_version(message_version: u32, policy: SchemaCompatibility) -> Result<()> {
    // Special case: version 0 treated as v1 (proto3 default)
    let message_version = if message_version == 0 {
        1
    } else {
        message_version
    };

    match policy {
        SchemaCompatibility::Exact => {
            if message_version != CURRENT_SCHEMA_VERSION {
                return Err(Error::InvalidFormat(format!(
                    "Schema version mismatch: expected v{}, got v{}",
                    CURRENT_SCHEMA_VERSION, message_version
                )));
            }
        }
        SchemaCompatibility::ForwardCompatible => {
            if message_version < CURRENT_SCHEMA_VERSION {
                return Err(Error::InvalidFormat(format!(
                    "Schema version too old: expected >= v{}, got v{}",
                    CURRENT_SCHEMA_VERSION, message_version
                )));
            }
        }
        SchemaCompatibility::BackwardCompatible => {
            if message_version > CURRENT_SCHEMA_VERSION {
                return Err(Error::InvalidFormat(format!(
                    "Schema version too new: expected <= v{}, got v{}",
                    CURRENT_SCHEMA_VERSION, message_version
                )));
            }
        }
    }
    Ok(())
}

/// Extract schema version from any message type
///
/// Returns the schema version from the message header, or None if no header found.
fn extract_schema_version(message: &DashStreamMessage) -> Option<u32> {
    match &message.message {
        Some(msg) => match msg {
            crate::dash_stream_message::Message::Event(e) => {
                e.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::TokenChunk(t) => {
                t.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::StateDiff(s) => {
                s.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::ToolExecution(te) => {
                te.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::Checkpoint(c) => {
                c.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::Metrics(m) => {
                m.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::Error(err) => {
                err.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::EventBatch(batch) => {
                batch.header.as_ref().map(|h| h.schema_version)
            }
            crate::dash_stream_message::Message::ExecutionTrace(trace) => {
                trace.header.as_ref().map(|h| h.schema_version)
            }
        },
        None => None,
    }
}

/// Decode message with schema version validation
///
/// # Arguments
///
/// * `data` - The bytes to decode
/// * `policy` - The schema compatibility policy
///
/// # Returns
///
/// The decoded DashStreamMessage if schema version is compatible
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_streaming::codec::{decode_message_with_validation, SchemaCompatibility};
///
/// # let data = vec![];
/// match decode_message_with_validation(&data, SchemaCompatibility::Exact) {
///     Ok(msg) => println!("Valid message"),
///     Err(e) => eprintln!("Schema validation failed: {}", e),
/// }
/// ```
pub fn decode_message_with_validation(
    data: &[u8],
    policy: SchemaCompatibility,
) -> Result<DashStreamMessage> {
    decode_message_with_validation_strict(data, policy, true)
}

/// Decode message with schema validation and strict mode option
///
/// # Arguments
///
/// * `data` - The bytes to decode
/// * `policy` - The schema compatibility policy
/// * `require_header` - If true, reject messages without headers (security mode)
///
/// # Security
///
/// When `require_header` is true, messages without headers are rejected.
/// This prevents attackers from stripping headers to bypass schema validation.
///
/// For EventBatch messages, both the batch header AND each individual event's
/// header are validated for consistent batch validation.
pub fn decode_message_with_validation_strict(
    data: &[u8],
    policy: SchemaCompatibility,
    require_header: bool,
) -> Result<DashStreamMessage> {
    if data.is_empty() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Empty message payload",
        )));
    }

    // Support both framed (0x00/0x01) and legacy raw protobuf messages.
    // Protobuf messages cannot begin with 0x00/0x01 (field number 0 is invalid),
    // so this is a safe disambiguation.
    let message = match data[0] {
        HEADER_UNCOMPRESSED => {
            let framed_max = DEFAULT_MAX_PAYLOAD_SIZE.saturating_add(1);
            if data.len() > framed_max {
                return Err(Error::InvalidFormat(format!(
                    "Payload size {} bytes exceeds maximum {} bytes",
                    data.len(),
                    framed_max
                )));
            }
            decode_message(&data[1..])?
        }
        HEADER_COMPRESSED_ZSTD => {
            let framed_max = DEFAULT_MAX_PAYLOAD_SIZE.saturating_add(1);
            if data.len() > framed_max {
                return Err(Error::InvalidFormat(format!(
                    "Payload size {} bytes exceeds maximum {} bytes",
                    data.len(),
                    framed_max
                )));
            }
            let decompressed = crate::compression::decompress_zstd_with_limit(
                &data[1..],
                DEFAULT_MAX_PAYLOAD_SIZE,
            )?;
            decode_message(&decompressed)?
        }
        _ => {
            if data.len() > DEFAULT_MAX_PAYLOAD_SIZE {
                return Err(Error::InvalidFormat(format!(
                    "Payload size {} bytes exceeds maximum {} bytes",
                    data.len(),
                    DEFAULT_MAX_PAYLOAD_SIZE
                )));
            }
            decode_message(data)?
        }
    };

    // Extract and validate schema version from message header
    if let Some(version) = extract_schema_version(&message) {
        validate_schema_version(version, policy)?;
    } else if require_header {
        // Security: Reject messages without headers to prevent header-stripping attacks
        return Err(Error::InvalidFormat(
            "Message missing required header with schema version. \
             This could indicate a header-stripping attack or malformed message."
                .to_string(),
        ));
    }
    // If !require_header and no header, silently allow (legacy compatibility mode)

    // Validate events within EventBatch for consistent schema enforcement
    validate_batch_events(&message, policy, require_header)?;

    Ok(message)
}

/// Validate individual events within an EventBatch.
///
/// Ensures that all events within a batch have valid schema versions,
/// providing consistent validation between batch and single-message paths.
fn validate_batch_events(
    message: &DashStreamMessage,
    policy: SchemaCompatibility,
    require_header: bool,
) -> Result<()> {
    if let Some(crate::dash_stream_message::Message::EventBatch(batch)) = &message.message {
        for (index, event) in batch.events.iter().enumerate() {
            if let Some(header) = &event.header {
                validate_schema_version(header.schema_version, policy).map_err(|e| {
                    Error::InvalidFormat(format!(
                        "EventBatch event[{}] schema validation failed: {}",
                        index, e
                    ))
                })?;
            } else if require_header {
                return Err(Error::InvalidFormat(format!(
                    "EventBatch event[{}] missing required header with schema version. \
                     All events in a batch must have headers when require_header is enabled.",
                    index
                )));
            }
        }
    }
    Ok(())
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Event, EventType, Header, MessageType};

    fn create_test_event() -> Event {
        Event {
            header: Some(Header {
                message_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                timestamp_us: 1234567890,
                tenant_id: "test-tenant".to_string(),
                thread_id: "test-thread".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "start".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        }
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event.clone())),
        };

        // Encode
        let bytes = encode_message(&message).unwrap();
        assert!(!bytes.is_empty());

        // Decode
        let decoded = decode_message(&bytes).unwrap();

        // Verify
        match decoded.message {
            Some(crate::dash_stream_message::Message::Event(decoded_event)) => {
                assert_eq!(decoded_event.event_type, event.event_type);
                assert_eq!(decoded_event.node_id, event.node_id);
                assert_eq!(
                    decoded_event.header.as_ref().unwrap().thread_id,
                    event.header.as_ref().unwrap().thread_id
                );
            }
            _ => panic!("Expected Event message"),
        }
    }

    #[test]
    fn test_encode_with_compression() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode without compression
        let (bytes_uncompressed, is_compressed) =
            encode_message_with_compression(&message, false).unwrap();
        assert!(!is_compressed);

        // Encode with compression (but message is too small, so won't be compressed)
        let (bytes_maybe_compressed, was_compressed) =
            encode_message_with_compression(&message, true).unwrap();

        // Message is small, so it won't be compressed
        assert!(!was_compressed);
        assert_eq!(bytes_uncompressed.len(), bytes_maybe_compressed.len());
    }

    #[test]
    fn test_decode_message_compatible_accepts_legacy_unframed() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode
        let bytes = encode_message(&message).unwrap();

        // Decode legacy unframed payload via the compatibility helper.
        let decoded = decode_message_compatible(&bytes, DEFAULT_MAX_PAYLOAD_SIZE).unwrap();
        assert!(matches!(
            decoded.message,
            Some(crate::dash_stream_message::Message::Event(_))
        ));
    }

    #[test]
    fn test_decode_message_compatible_accepts_framed_and_unframed() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        let raw = encode_message(&message).unwrap();
        let decoded_raw = decode_message_compatible(&raw, DEFAULT_MAX_PAYLOAD_SIZE).unwrap();
        assert!(matches!(
            decoded_raw.message,
            Some(crate::dash_stream_message::Message::Event(_))
        ));

        let mut framed = Vec::with_capacity(raw.len() + 1);
        framed.push(HEADER_UNCOMPRESSED);
        framed.extend_from_slice(&raw);
        let decoded_framed = decode_message_compatible(&framed, DEFAULT_MAX_PAYLOAD_SIZE).unwrap();
        assert!(matches!(
            decoded_framed.message,
            Some(crate::dash_stream_message::Message::Event(_))
        ));
    }

    #[test]
    fn test_decode_message_compatible_decompresses_zstd_magic() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        let raw = encode_message(&message).unwrap();
        let compressed = crate::compression::compress_zstd(&raw, 3).unwrap();
        let decoded = decode_message_compatible(&compressed, DEFAULT_MAX_PAYLOAD_SIZE).unwrap();
        assert!(matches!(
            decoded.message,
            Some(crate::dash_stream_message::Message::Event(_))
        ));
    }

    #[test]
    fn test_encode_empty_message() {
        let message = DashStreamMessage { message: None };
        let bytes = encode_message(&message).unwrap();
        let decoded = decode_message(&bytes).unwrap();
        assert!(decoded.message.is_none());
    }

    #[test]
    fn test_decode_invalid_bytes() {
        // Test decoding invalid protobuf data
        let invalid_bytes = vec![0xFF, 0xFF, 0xFF, 0xFF];
        let result = decode_message(&invalid_bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_fallback_on_small_message() {
        // Create a small message (< 512 bytes)
        let event = Event {
            header: Some(Header {
                message_id: vec![1, 2, 3, 4],
                timestamp_us: 123,
                tenant_id: "t".to_string(),
                thread_id: "th".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "n".to_string(),
            attributes: Default::default(),
            duration_us: 0,
            llm_request_id: "".to_string(),
        };
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode with compression=true, but message is small
        let (bytes, is_compressed) = encode_message_with_compression(&message, true).unwrap();

        // Should not be compressed because message < 512 bytes
        assert!(!is_compressed);
        assert!(bytes.len() < 512);
    }

    #[test]
    fn test_large_message_compression() {
        use crate::{attribute_value::Value as AttrVal, AttributeValue};

        // Create a large message (> 512 bytes) with highly compressible data
        let mut large_attributes = std::collections::HashMap::new();
        for i in 0..50 {
            large_attributes.insert(
                format!("key_{}", i),
                AttributeValue {
                    value: Some(AttrVal::StringValue(format!(
                        "value_with_repeated_content_repeated_content_repeated_content_{}",
                        i
                    ))),
                },
            );
        }

        let event = Event {
            header: Some(Header {
                message_id: vec![1; 16],
                timestamp_us: 1234567890,
                tenant_id: "test-tenant-with-long-name".to_string(),
                thread_id: "test-thread-with-long-name".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "node-with-long-identifier".to_string(),
            attributes: large_attributes,
            duration_us: 1234567,
            llm_request_id: "llm-request-id-with-long-identifier".to_string(),
        };
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode without compression
        let (bytes_uncompressed, is_compressed) =
            encode_message_with_compression(&message, false).unwrap();
        assert!(!is_compressed);
        assert!(bytes_uncompressed.len() > 512); // Verify it's actually large

        // Encode with compression
        let (bytes_compressed, is_compressed) =
            encode_message_with_compression(&message, true).unwrap();

        // Should be compressed and smaller
        assert!(is_compressed);
        assert!(bytes_compressed.len() < bytes_uncompressed.len());

        // Verify roundtrip
        let decoded = decode_message_strict(&bytes_compressed, DEFAULT_MAX_PAYLOAD_SIZE).unwrap();
        assert!(matches!(
            decoded.message,
            Some(crate::dash_stream_message::Message::Event(_))
        ));
    }

    #[test]
    fn test_decode_compressed_message_roundtrip() {
        use crate::{attribute_value::Value as AttrVal, AttributeValue};

        // Create a large compressible message
        let mut large_attributes = std::collections::HashMap::new();
        for i in 0..30 {
            large_attributes.insert(
                format!("key_{}", i),
                AttributeValue {
                    value: Some(AttrVal::StringValue("repeated_value".to_string())),
                },
            );
        }

        let event = Event {
            header: Some(Header {
                message_id: vec![1; 16],
                timestamp_us: 1234567890,
                tenant_id: "tenant".to_string(),
                thread_id: "thread".to_string(),
                sequence: 1,
                r#type: MessageType::Event as i32,
                parent_id: vec![],
                compression: 0,
                schema_version: 1,
            }),
            event_type: EventType::GraphStart as i32,
            node_id: "node".to_string(),
            attributes: large_attributes,
            duration_us: 0,
            llm_request_id: "".to_string(),
        };
        let original_message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode with compression
        let (compressed_bytes, is_compressed) =
            encode_message_with_compression(&original_message, true).unwrap();
        assert!(is_compressed);

        // Decode using strict framing (compression header required).
        let decoded_message = decode_message_strict(&compressed_bytes, DEFAULT_MAX_PAYLOAD_SIZE)
            .unwrap();

        // Verify content matches
        match (original_message.message, decoded_message.message) {
            (
                Some(crate::dash_stream_message::Message::Event(orig)),
                Some(crate::dash_stream_message::Message::Event(decoded)),
            ) => {
                assert_eq!(orig.event_type, decoded.event_type);
                assert_eq!(orig.node_id, decoded.node_id);
                assert_eq!(orig.attributes.len(), decoded.attributes.len());
            }
            _ => panic!("Expected Event messages"),
        }
    }

    // ============================================================================
    // Schema Validation Tests
    // ============================================================================

    #[test]
    fn test_schema_version_exact_match() {
        // v1 message, Exact policy -> OK
        assert!(validate_schema_version(1, SchemaCompatibility::Exact).is_ok());

        // v2 message, Exact policy -> ERROR
        assert!(validate_schema_version(2, SchemaCompatibility::Exact).is_err());
    }

    #[test]
    fn test_schema_version_forward_compatible() {
        // v1 consumer can read v1 and v2
        assert!(validate_schema_version(1, SchemaCompatibility::ForwardCompatible).is_ok());
        assert!(validate_schema_version(2, SchemaCompatibility::ForwardCompatible).is_ok());

        // Version 0 treated as v1
        assert!(validate_schema_version(0, SchemaCompatibility::ForwardCompatible).is_ok());
    }

    #[test]
    fn test_schema_version_backward_compatible() {
        // v1 consumer can read v0 and v1
        assert!(validate_schema_version(0, SchemaCompatibility::BackwardCompatible).is_ok());
        assert!(validate_schema_version(1, SchemaCompatibility::BackwardCompatible).is_ok());

        // v2 is too new
        assert!(validate_schema_version(2, SchemaCompatibility::BackwardCompatible).is_err());
    }

    #[test]
    fn test_schema_version_zero_treated_as_v1() {
        // Version 0 (proto3 default) treated as v1
        assert!(validate_schema_version(0, SchemaCompatibility::Exact).is_ok());
    }

    #[test]
    fn test_decode_with_validation_success() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };
        let bytes = encode_message(&message).unwrap();

        // Should decode successfully with Exact policy (v1 == v1)
        let decoded = decode_message_with_validation(&bytes, SchemaCompatibility::Exact);
        assert!(decoded.is_ok());
    }

    #[test]
    fn test_decode_with_validation_version_mismatch() {
        // Create a v2 message by manually setting schema_version
        let mut event = create_test_event();
        if let Some(ref mut header) = event.header {
            header.schema_version = 2; // Simulate v2 message
        }

        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };
        let bytes = encode_message(&message).unwrap();

        // Should fail with Exact policy (v1 consumer, v2 message)
        let result = decode_message_with_validation(&bytes, SchemaCompatibility::Exact);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Schema version mismatch"));
    }

    #[test]
    fn test_extract_schema_version() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        let version = extract_schema_version(&message);
        assert_eq!(version, Some(1));
    }

    #[test]
    fn test_schema_compatibility_default() {
        let policy = SchemaCompatibility::default();
        assert_eq!(policy, SchemaCompatibility::Exact);
    }

    #[test]
    fn test_decode_message_strict_accepts_valid_headers() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Encode with compression header (0x00)
        let (bytes, _) = encode_message_with_compression(&message, false).unwrap();
        assert_eq!(bytes[0], HEADER_UNCOMPRESSED);

        // Strict decode should accept valid header
        let decoded = decode_message_strict(&bytes, DEFAULT_MAX_PAYLOAD_SIZE).unwrap();
        assert!(matches!(
            decoded.message,
            Some(crate::dash_stream_message::Message::Event(_))
        ));
    }

    #[test]
    fn test_decode_message_strict_rejects_invalid_header() {
        // Construct a payload with invalid first byte (not 0x00 or 0x01)
        let invalid_payloads: Vec<(u8, &str)> = vec![
            (0x02, "0x02"),
            (0xFF, "0xFF"),
            (0x10, "0x10"),
            (0x80, "0x80"),
        ];

        for (first_byte, desc) in invalid_payloads {
            let mut payload = vec![first_byte];
            payload.extend_from_slice(b"some data");

            let result = decode_message_strict(&payload, DEFAULT_MAX_PAYLOAD_SIZE);
            assert!(
                result.is_err(),
                "Strict decode should reject payload with {} header",
                desc
            );

            let err = result.unwrap_err();
            let err_msg = err.to_string();
            assert!(
                err_msg.contains("Invalid compression header byte"),
                "Error should mention invalid header byte for {}: {}",
                desc,
                err_msg
            );
        }
    }

    #[test]
    fn test_decode_message_strict_rejects_empty() {
        let result = decode_message_strict(&[], DEFAULT_MAX_PAYLOAD_SIZE);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Empty"));
    }

    #[test]
    fn test_decode_message_strict_enforces_size_limit() {
        let large_payload = vec![0x00; 1000];
        let result = decode_message_strict(&large_payload, 100);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_header_constants() {
        // Verify constants match expected values
        assert_eq!(HEADER_UNCOMPRESSED, 0x00);
        assert_eq!(HEADER_COMPRESSED_ZSTD, 0x01);
    }

    // ============================================================================
    // Buffer Pool Tests
    // ============================================================================

    #[test]
    fn test_encode_message_pooled_roundtrip() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event.clone())),
        };

        // Encode with pooled function
        let bytes = encode_message_pooled(&message).unwrap();
        assert!(!bytes.is_empty());

        // Decode and verify
        let decoded = decode_message(&bytes).unwrap();
        match decoded.message {
            Some(crate::dash_stream_message::Message::Event(decoded_event)) => {
                assert_eq!(decoded_event.event_type, event.event_type);
                assert_eq!(decoded_event.node_id, event.node_id);
            }
            _ => panic!("Expected Event message"),
        }
    }

    #[test]
    fn test_encode_message_pooled_matches_standard() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        // Both encoding methods should produce identical output
        let standard_bytes = encode_message(&message).unwrap();
        let pooled_bytes = encode_message_pooled(&message).unwrap();

        assert_eq!(standard_bytes, pooled_bytes);
    }

    #[test]
    fn test_encode_message_pooled_multiple_calls() {
        // Verify buffer reuse across multiple calls
        for i in 0..100 {
            let event = Event {
                header: Some(Header {
                    message_id: vec![i as u8; 16],
                    timestamp_us: i as i64,
                    tenant_id: format!("tenant-{}", i),
                    thread_id: format!("thread-{}", i),
                    sequence: i as u64,
                    r#type: MessageType::Event as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1,
                }),
                event_type: EventType::GraphStart as i32,
                node_id: format!("node-{}", i),
                attributes: Default::default(),
                duration_us: i as i64,
                llm_request_id: "".to_string(),
            };
            let message = DashStreamMessage {
                message: Some(crate::dash_stream_message::Message::Event(event)),
            };

            let bytes = encode_message_pooled(&message).unwrap();
            assert!(!bytes.is_empty());

            // Verify roundtrip
            let decoded = decode_message(&bytes).unwrap();
            assert!(decoded.message.is_some());
        }
    }

    #[test]
    fn test_encode_message_into_roundtrip() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event.clone())),
        };

        let mut buf = Vec::with_capacity(4096);
        let len = encode_message_into(&message, &mut buf).unwrap();

        assert_eq!(len, buf.len());
        assert!(!buf.is_empty());

        // Decode and verify
        let decoded = decode_message(&buf).unwrap();
        match decoded.message {
            Some(crate::dash_stream_message::Message::Event(decoded_event)) => {
                assert_eq!(decoded_event.event_type, event.event_type);
                assert_eq!(decoded_event.node_id, event.node_id);
            }
            _ => panic!("Expected Event message"),
        }
    }

    #[test]
    fn test_encode_message_into_multiple_reuses() {
        let mut buf = Vec::with_capacity(4096);

        // Reuse the same buffer for multiple encodes
        for i in 0..50 {
            let event = Event {
                header: Some(Header {
                    message_id: vec![i as u8; 16],
                    timestamp_us: i as i64,
                    tenant_id: "test-tenant".to_string(),
                    thread_id: "test-thread".to_string(),
                    sequence: i as u64,
                    r#type: MessageType::Event as i32,
                    parent_id: vec![],
                    compression: 0,
                    schema_version: 1,
                }),
                event_type: EventType::GraphStart as i32,
                node_id: format!("node-{}", i),
                attributes: Default::default(),
                duration_us: 0,
                llm_request_id: "".to_string(),
            };
            let message = DashStreamMessage {
                message: Some(crate::dash_stream_message::Message::Event(event)),
            };

            let len = encode_message_into(&message, &mut buf).unwrap();
            assert_eq!(len, buf.len());

            // Verify roundtrip
            let decoded = decode_message(&buf).unwrap();
            assert!(decoded.message.is_some());
        }
    }

    #[test]
    fn test_encode_message_into_matches_standard() {
        let event = create_test_event();
        let message = DashStreamMessage {
            message: Some(crate::dash_stream_message::Message::Event(event)),
        };

        let standard_bytes = encode_message(&message).unwrap();

        let mut buf = Vec::new();
        encode_message_into(&message, &mut buf).unwrap();

        assert_eq!(standard_bytes, buf);
    }

    #[test]
    fn test_buffer_pool_constants() {
        assert_eq!(DEFAULT_BUFFER_CAPACITY, 4096);
        assert_eq!(MAX_POOL_SIZE, 8);
    }
}
