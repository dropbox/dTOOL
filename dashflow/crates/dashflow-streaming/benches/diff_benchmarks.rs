use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow_streaming::diff::{apply_patch, diff_states};
use serde_json::json;

fn bench_diff_small_state(c: &mut Criterion) {
    let old = json!({
        "counter": 5,
        "message": "Hello"
    });
    let new = json!({
        "counter": 6,
        "message": "Hello"
    });

    c.bench_function("diff_small_state", |b| {
        b.iter(|| diff_states(black_box(&old), black_box(&new)));
    });
}

fn bench_diff_medium_state(c: &mut Criterion) {
    let old = json!({
        "messages": vec!["Hello", "How are you?", "I'm fine"],
        "counter": 5,
        "metadata": {
            "timestamp": 1234567890,
            "user": "Alice",
            "session": "abc123"
        },
        "flags": {
            "active": true,
            "debug": false
        }
    });
    let new = json!({
        "messages": vec!["Hello", "How are you?", "I'm fine", "Goodbye"],
        "counter": 6,
        "metadata": {
            "timestamp": 1234567900,
            "user": "Alice",
            "session": "abc123"
        },
        "flags": {
            "active": true,
            "debug": false
        }
    });

    c.bench_function("diff_medium_state", |b| {
        b.iter(|| diff_states(black_box(&old), black_box(&new)));
    });
}

fn bench_diff_large_state(c: &mut Criterion) {
    let old = json!({
        "messages": (0..50).map(|i| format!("Message {}", i)).collect::<Vec<_>>(),
        "counter": 5,
        "metadata": {
            "timestamp": 1234567890,
            "user": "Alice",
            "session": "abc123",
            "attributes": (0..20).map(|i| (format!("attr{}", i), serde_json::Value::from(i))).collect::<serde_json::Map<_, _>>()
        },
        "flags": {
            "active": true,
            "debug": false
        }
    });
    let new = json!({
        "messages": (0..51).map(|i| format!("Message {}", i)).collect::<Vec<_>>(),
        "counter": 6,
        "metadata": {
            "timestamp": 1234567900,
            "user": "Alice",
            "session": "abc123",
            "attributes": (0..20).map(|i| (format!("attr{}", i), serde_json::Value::from(i + 1))).collect::<serde_json::Map<_, _>>()
        },
        "flags": {
            "active": true,
            "debug": false
        }
    });

    c.bench_function("diff_large_state", |b| {
        b.iter(|| diff_states(black_box(&old), black_box(&new)));
    });
}

fn bench_apply_patch_small(c: &mut Criterion) {
    let old = json!({
        "counter": 5,
        "message": "Hello"
    });
    let new = json!({
        "counter": 6,
        "message": "Hello"
    });

    let result = match diff_states(&old, &new) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("Skipping apply_patch_small benchmark (diff failed): {err}");
            c.bench_function("apply_patch_small", |b| b.iter(|| ()));
            return;
        }
    };

    c.bench_function("apply_patch_small", |b| {
        b.iter(|| apply_patch(black_box(&old), black_box(&result.patch)));
    });
}

fn bench_apply_patch_medium(c: &mut Criterion) {
    let old = json!({
        "messages": vec!["Hello", "How are you?", "I'm fine"],
        "counter": 5,
        "metadata": {
            "timestamp": 1234567890,
            "user": "Alice",
            "session": "abc123"
        },
        "flags": {
            "active": true,
            "debug": false
        }
    });
    let new = json!({
        "messages": vec!["Hello", "How are you?", "I'm fine", "Goodbye"],
        "counter": 6,
        "metadata": {
            "timestamp": 1234567900,
            "user": "Alice",
            "session": "abc123"
        },
        "flags": {
            "active": true,
            "debug": false
        }
    });

    let result = match diff_states(&old, &new) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("Skipping apply_patch_medium benchmark (diff failed): {err}");
            c.bench_function("apply_patch_medium", |b| b.iter(|| ()));
            return;
        }
    };

    c.bench_function("apply_patch_medium", |b| {
        b.iter(|| apply_patch(black_box(&old), black_box(&result.patch)));
    });
}

fn bench_diff_vs_full_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_vs_full_state");

    for size in [10, 50, 100, 200].iter() {
        let old = json!({
            "messages": (0..*size).map(|i| format!("Message {}", i)).collect::<Vec<_>>(),
            "counter": 5,
        });
        let new = json!({
            "messages": (0..*size + 1).map(|i| format!("Message {}", i)).collect::<Vec<_>>(),
            "counter": 6,
        });

        group.bench_with_input(BenchmarkId::new("diff", size), size, |b, _| {
            b.iter(|| diff_states(black_box(&old), black_box(&new)));
        });
    }

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let old = json!({
        "messages": vec!["Hello", "How are you?", "I'm fine"],
        "counter": 5,
        "metadata": {
            "timestamp": 1234567890,
            "user": "Alice",
            "session": "abc123"
        }
    });
    let new = json!({
        "messages": vec!["Hello", "How are you?", "I'm fine", "Goodbye"],
        "counter": 6,
        "metadata": {
            "timestamp": 1234567900,
            "user": "Alice",
            "session": "abc123"
        }
    });

    c.bench_function("diff_apply_roundtrip", |b| {
        b.iter(|| {
            if let Ok(result) = diff_states(black_box(&old), black_box(&new)) {
                let _ = black_box(apply_patch(black_box(&old), black_box(&result.patch)));
            }
        });
    });
}

criterion_group!(
    benches,
    bench_diff_small_state,
    bench_diff_medium_state,
    bench_diff_large_state,
    bench_apply_patch_small,
    bench_apply_patch_medium,
    bench_diff_vs_full_state,
    bench_roundtrip
);
criterion_main!(benches);
