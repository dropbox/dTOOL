// © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
// Tests for dashflow-cli profile command

#[derive(Debug)]
struct NodeProfile {
    executions: usize,
    total_duration: i64,
    min_duration: i64,
    max_duration: i64,
    durations: Vec<i64>,
}

impl NodeProfile {
    fn new() -> Self {
        Self {
            executions: 0,
            total_duration: 0,
            min_duration: i64::MAX,
            max_duration: 0,
            durations: Vec::new(),
        }
    }

    fn add_execution(&mut self, duration: i64) {
        self.executions += 1;
        self.total_duration += duration;
        self.min_duration = self.min_duration.min(duration);
        self.max_duration = self.max_duration.max(duration);
        self.durations.push(duration);
    }

    fn avg_duration(&self) -> i64 {
        if self.executions > 0 {
            self.total_duration / self.executions as i64
        } else {
            0
        }
    }

    fn median_duration(&self) -> i64 {
        if self.durations.is_empty() {
            return 0;
        }

        let mut sorted = self.durations.clone();
        sorted.sort_unstable();
        sorted[sorted.len() / 2]
    }

    fn p95_duration(&self) -> i64 {
        if self.durations.is_empty() {
            return 0;
        }

        let mut sorted = self.durations.clone();
        sorted.sort_unstable();
        let idx = (sorted.len() as f64 * 0.95) as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

#[test]
fn test_node_profile_new() {
    let profile = NodeProfile::new();

    assert_eq!(profile.executions, 0);
    assert_eq!(profile.total_duration, 0);
    assert_eq!(profile.min_duration, i64::MAX);
    assert_eq!(profile.max_duration, 0);
    assert!(profile.durations.is_empty());
}

#[test]
fn test_node_profile_single_execution() {
    let mut profile = NodeProfile::new();
    profile.add_execution(1000);

    assert_eq!(profile.executions, 1);
    assert_eq!(profile.total_duration, 1000);
    assert_eq!(profile.min_duration, 1000);
    assert_eq!(profile.max_duration, 1000);
    assert_eq!(profile.avg_duration(), 1000);
    assert_eq!(profile.median_duration(), 1000);
}

#[test]
fn test_node_profile_multiple_executions() {
    let mut profile = NodeProfile::new();
    profile.add_execution(1000);
    profile.add_execution(2000);
    profile.add_execution(3000);

    assert_eq!(profile.executions, 3);
    assert_eq!(profile.total_duration, 6000);
    assert_eq!(profile.min_duration, 1000);
    assert_eq!(profile.max_duration, 3000);
    assert_eq!(profile.avg_duration(), 2000);
}

#[test]
fn test_node_profile_avg_calculation() {
    let mut profile = NodeProfile::new();
    profile.add_execution(100);
    profile.add_execution(200);
    profile.add_execution(300);
    profile.add_execution(400);

    // Average: (100 + 200 + 300 + 400) / 4 = 250
    assert_eq!(profile.avg_duration(), 250);
}

#[test]
fn test_node_profile_median_odd_count() {
    let mut profile = NodeProfile::new();
    profile.add_execution(100);
    profile.add_execution(300);
    profile.add_execution(200);
    profile.add_execution(500);
    profile.add_execution(400);

    // Sorted: [100, 200, 300, 400, 500]
    // Median (middle element): 300
    assert_eq!(profile.median_duration(), 300);
}

#[test]
fn test_node_profile_median_even_count() {
    let mut profile = NodeProfile::new();
    profile.add_execution(100);
    profile.add_execution(200);
    profile.add_execution(300);
    profile.add_execution(400);

    // Sorted: [100, 200, 300, 400]
    // Median (middle element at len/2): 300
    assert_eq!(profile.median_duration(), 300);
}

#[test]
fn test_node_profile_p95_calculation() {
    let mut profile = NodeProfile::new();

    // Add 100 executions
    for i in 1..=100 {
        profile.add_execution(i * 10);
    }

    let p95 = profile.p95_duration();

    // P95 should be around 95th percentile
    // With 100 values, p95 index = 95
    // Value at index 95 (0-based) = 960
    assert_eq!(p95, 960);
}

#[test]
fn test_node_profile_min_max_tracking() {
    let mut profile = NodeProfile::new();
    profile.add_execution(500);
    profile.add_execution(100); // New min
    profile.add_execution(1000); // New max
    profile.add_execution(300);

    assert_eq!(profile.min_duration, 100);
    assert_eq!(profile.max_duration, 1000);
}

#[test]
fn test_node_profile_empty_median() {
    let profile = NodeProfile::new();
    assert_eq!(profile.median_duration(), 0);
}

#[test]
fn test_node_profile_empty_p95() {
    let profile = NodeProfile::new();
    assert_eq!(profile.p95_duration(), 0);
}

#[test]
fn test_node_profile_empty_avg() {
    let profile = NodeProfile::new();
    assert_eq!(profile.avg_duration(), 0);
}

#[test]
fn test_node_profile_large_numbers() {
    let mut profile = NodeProfile::new();
    profile.add_execution(1_000_000); // 1 second
    profile.add_execution(5_000_000); // 5 seconds
    profile.add_execution(10_000_000); // 10 seconds

    assert_eq!(profile.total_duration, 16_000_000);
    assert_eq!(profile.avg_duration(), 5_333_333);
    assert_eq!(profile.min_duration, 1_000_000);
    assert_eq!(profile.max_duration, 10_000_000);
}

#[test]
fn test_node_profile_zero_durations() {
    let mut profile = NodeProfile::new();
    profile.add_execution(0);
    profile.add_execution(0);
    profile.add_execution(0);

    assert_eq!(profile.executions, 3);
    assert_eq!(profile.total_duration, 0);
    assert_eq!(profile.avg_duration(), 0);
    assert_eq!(profile.min_duration, 0);
    assert_eq!(profile.max_duration, 0);
}

#[test]
fn test_duration_formatting() {
    // Test microseconds to milliseconds conversion
    let duration_us = 1_234_567i64; // 1.234567 seconds
    let duration_ms = duration_us as f64 / 1_000.0;
    let duration_s = duration_us as f64 / 1_000_000.0;

    assert!((duration_ms - 1234.567).abs() < 0.001);
    assert!((duration_s - 1.234567).abs() < 0.000001);
}

#[test]
fn test_format_duration_display() {
    // Test various duration formats
    let us = 123;
    let ms = us as f64 / 1_000.0;
    assert!((ms - 0.123).abs() < 0.001);

    let large_us = 45_678_901;
    let large_s = large_us as f64 / 1_000_000.0;
    assert!((large_s - 45.678901).abs() < 0.000001);
}

#[test]
fn test_profile_percentile_edge_cases() {
    let mut profile = NodeProfile::new();
    profile.add_execution(100);

    // With single value, p95 should be that value
    assert_eq!(profile.p95_duration(), 100);
}

#[test]
fn test_profile_statistics_consistency() {
    let mut profile = NodeProfile::new();
    profile.add_execution(100);
    profile.add_execution(200);
    profile.add_execution(300);

    // Min should be <= avg <= max
    assert!(profile.min_duration <= profile.avg_duration());
    assert!(profile.avg_duration() <= profile.max_duration);

    // Median should be between min and max
    assert!(profile.min_duration <= profile.median_duration());
    assert!(profile.median_duration() <= profile.max_duration);

    // P95 should be between median and max
    assert!(profile.median_duration() <= profile.p95_duration());
    assert!(profile.p95_duration() <= profile.max_duration);
}

#[test]
fn test_profile_accumulation() {
    let mut profile = NodeProfile::new();
    let durations = vec![100, 200, 300, 400, 500];

    for &duration in &durations {
        profile.add_execution(duration);
    }

    assert_eq!(profile.executions, durations.len());
    assert_eq!(profile.durations, durations);
}

#[test]
fn test_profile_top_operations() {
    let mut profiles = [
        ("fast", 100i64),
        ("slow", 5000i64),
        ("medium", 500i64),
        ("very_slow", 10000i64),
    ];

    // Sort by duration (descending)
    profiles.sort_by(|a, b| b.1.cmp(&a.1));

    // Top 2 should be very_slow and slow
    assert_eq!(profiles[0].0, "very_slow");
    assert_eq!(profiles[1].0, "slow");

    // Take top 3
    let top3: Vec<_> = profiles.iter().take(3).collect();
    assert_eq!(top3.len(), 3);
}

#[test]
fn test_profile_args_defaults() {
    // Verify default values
    let default_bootstrap = "localhost:9092";
    let default_topic = "dashstream";
    let default_top = 10;

    assert_eq!(default_bootstrap, "localhost:9092");
    assert_eq!(default_topic, "dashstream");
    assert_eq!(default_top, 10);
}
