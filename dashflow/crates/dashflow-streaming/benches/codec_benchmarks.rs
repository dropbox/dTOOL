// Allow deprecated: Benchmarks measure performance of both current and deprecated functions
#![allow(deprecated)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow_streaming::{
    codec::{
        decode_message, decode_message_with_decompression, encode_message,
        encode_message_with_compression,
    },
    compression::{compress_zstd, decompress_zstd},
    DashStreamMessage, Event, EventType, Header, MessageType, StateDiff,
};

fn create_test_event() -> Event {
    Event {
        header: Some(Header {
            message_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            timestamp_us: 1234567890,
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread-12345".to_string(),
            sequence: 1,
            r#type: MessageType::Event as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        event_type: EventType::GraphStart as i32,
        node_id: "start_node".to_string(),
        attributes: Default::default(),
        duration_us: 0,
        llm_request_id: "llm-request-123".to_string(),
    }
}

fn create_large_state_diff() -> StateDiff {
    use dashflow_streaming::{
        diff_operation::OpType, diff_operation::ValueEncoding, DiffOperation,
    };

    let mut operations = Vec::new();
    for i in 0..100 {
        operations.push(DiffOperation {
            op: OpType::Add as i32,
            path: format!("/messages/{}/content", i),
            value: format!("Message content number {}", i).into_bytes(),
            from: String::new(),
            encoding: ValueEncoding::Json as i32,
        });
    }

    StateDiff {
        header: Some(Header {
            message_id: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            timestamp_us: 1234567890,
            tenant_id: "test-tenant".to_string(),
            thread_id: "test-thread".to_string(),
            sequence: 1,
            r#type: MessageType::StateDiff as i32,
            parent_id: vec![],
            compression: 0,
            schema_version: 1,
        }),
        base_checkpoint_id: vec![9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 1, 2, 3, 4, 5, 6],
        operations,
        state_hash: vec![0; 32],
        full_state: vec![],
    }
}

fn benchmark_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    // Event encoding
    let event = create_test_event();
    let event_msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };
    group.bench_function("event", |b| {
        b.iter(|| encode_message(black_box(&event_msg)))
    });

    // StateDiff encoding (larger message)
    let state_diff = create_large_state_diff();
    let state_diff_msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::StateDiff(
            state_diff,
        )),
    };
    group.bench_function("state_diff_large", |b| {
        b.iter(|| encode_message(black_box(&state_diff_msg)))
    });

    group.finish();
}

fn benchmark_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    // Event decoding
    let event = create_test_event();
    let event_msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };
    let encoded = encode_message(&event_msg).unwrap_or_default();
    group.bench_function("event", |b| b.iter(|| decode_message(black_box(&encoded))));

    // StateDiff decoding
    let state_diff = create_large_state_diff();
    let state_diff_msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::StateDiff(
            state_diff,
        )),
    };
    let encoded = encode_message(&state_diff_msg).unwrap_or_default();
    group.bench_function("state_diff_large", |b| {
        b.iter(|| decode_message(black_box(&encoded)))
    });

    group.finish();
}

fn benchmark_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    let event = create_test_event();
    let msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::Event(
            event,
        )),
    };
    group.bench_function("event", |b| {
        b.iter(|| {
            let encoded = encode_message(black_box(&msg)).unwrap_or_default();
            decode_message(black_box(&encoded)).unwrap_or_default()
        })
    });

    group.finish();
}

fn benchmark_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    // Create repeatable data (compresses well)
    let data = b"Hello, world! This is a test message. ".repeat(100);

    for level in [1, 3, 5, 10] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("zstd_level_{}", level)),
            &level,
            |b, &level| b.iter(|| compress_zstd(black_box(&data), black_box(level))),
        );
    }

    // Decompress benchmark
    let compressed = compress_zstd(&data, 3).unwrap_or_default();
    group.bench_function("zstd_decompress", |b| {
        b.iter(|| decompress_zstd(black_box(&compressed)))
    });

    group.finish();
}

fn benchmark_compression_with_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode_with_compression");

    let state_diff = create_large_state_diff();
    let msg = DashStreamMessage {
        message: Some(dashflow_streaming::dash_stream_message::Message::StateDiff(
            state_diff,
        )),
    };

    group.bench_function("no_compression", |b| {
        b.iter(|| encode_message_with_compression(black_box(&msg), black_box(false)))
    });

    group.bench_function("with_compression", |b| {
        b.iter(|| encode_message_with_compression(black_box(&msg), black_box(true)))
    });

    // Decode with decompression
    let (encoded, is_compressed) =
        encode_message_with_compression(&msg, true).unwrap_or((Vec::new(), false));
    group.bench_function("decode_decompressed", |b| {
        b.iter(|| decode_message_with_decompression(black_box(&encoded), black_box(is_compressed)))
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_encode,
    benchmark_decode,
    benchmark_roundtrip,
    benchmark_compression,
    benchmark_compression_with_encoding
);
criterion_main!(benches);
