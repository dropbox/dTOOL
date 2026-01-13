//! Benchmarks for self_improvement module hot paths
//!
//! Run with: cargo bench --bench self_improvement_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use dashflow::self_improvement::{
    CapabilityGap, Citation, ConsensusBuilder, ExecutionPlan, GapCategory, GapManifestation,
    Impact, ImplementationStep, ImprovementProposal, IntrospectionReport, IntrospectionScope,
    IntrospectionStorage, MockReviewer, PlanCategory, ProposalSource,
};
use tempfile::tempdir;

/// Benchmark report serialization
fn bench_report_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("report_serialization");

    // Create reports of different sizes
    for num_gaps in [0, 5, 20, 50].iter() {
        let mut report = IntrospectionReport::new(IntrospectionScope::System);

        // Add capability gaps
        for i in 0..*num_gaps {
            report.add_capability_gap(
                CapabilityGap::new(
                    format!("Gap {}: Missing feature", i),
                    GapCategory::PerformanceGap {
                        bottleneck: format!("node_{}", i),
                    },
                    GapManifestation::SuboptimalPaths {
                        description: "Detected during analysis".to_string(),
                    },
                )
                .with_solution(format!("Implement optimization {}", i))
                .with_confidence(0.8)
                .with_impact(Impact::medium("Performance improvement")),
            );
        }

        // Add some plans
        for i in 0..(*num_gaps / 2).max(1) {
            report.execution_plans.push(
                ExecutionPlan::new(format!("Plan {}", i), PlanCategory::Optimization)
                    .with_description("Test plan description")
                    .with_priority((i % 3 + 1) as u8)
                    .with_steps(vec![
                        ImplementationStep::new(1, "Step 1")
                            .with_files(vec!["file.rs".to_string()]),
                        ImplementationStep::new(2, "Step 2")
                            .with_files(vec!["file2.rs".to_string()]),
                    ])
                    .with_success_criteria(vec!["Tests pass".to_string()]),
            );
        }

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("to_json", format!("{}_gaps", num_gaps)),
            &report,
            |b, report| {
                b.iter(|| {
                    if let Ok(json) = report.to_json() {
                        black_box(json);
                    }
                });
            },
        );

        let json = match report.to_json() {
            Ok(json) => json,
            Err(err) => {
                eprintln!("Skipping from_json benchmark (serialization failed): {err}");
                continue;
            }
        };
        group.bench_with_input(
            BenchmarkId::new("from_json", format!("{}_gaps", num_gaps)),
            &json,
            |b, json| {
                b.iter(|| {
                    if let Ok(report) = IntrospectionReport::from_json(json) {
                        black_box(report);
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("to_markdown", format!("{}_gaps", num_gaps)),
            &report,
            |b, report| {
                b.iter(|| black_box(report.to_markdown()));
            },
        );
    }

    group.finish();
}

/// Benchmark storage operations
fn bench_storage_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage_operations");
    group.sample_size(50); // Fewer samples due to file I/O

    // Create a test report
    let mut report = IntrospectionReport::new(IntrospectionScope::System);
    for i in 0..10 {
        report.add_capability_gap(
            CapabilityGap::new(
                format!("Gap {}", i),
                GapCategory::PerformanceGap {
                    bottleneck: format!("node_{}", i),
                },
                GapManifestation::SuboptimalPaths {
                    description: "Test".to_string(),
                },
            )
            .with_confidence(0.8),
        );
    }

    group.throughput(Throughput::Elements(1));

    // Benchmark save_report
    group.bench_function("save_report", |b| {
        let dir = match tempdir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("Skipping save_report benchmark (tempdir failed): {err}");
                b.iter(|| ());
                return;
            }
        };
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        if let Err(err) = storage.initialize() {
            eprintln!("Skipping save_report benchmark (init failed): {err}");
            b.iter(|| ());
            return;
        }

        b.iter(|| {
            // Create new report each time to avoid overwriting
            let mut new_report = report.clone();
            new_report.id = uuid::Uuid::new_v4();
            let _ = black_box(storage.save_report(&new_report));
        });
    });

    // Benchmark load_report
    group.bench_function("load_report", |b| {
        let dir = match tempdir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("Skipping load_report benchmark (tempdir failed): {err}");
                b.iter(|| ());
                return;
            }
        };
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        if let Err(err) = storage.initialize() {
            eprintln!("Skipping load_report benchmark (init failed): {err}");
            b.iter(|| ());
            return;
        }

        // Save a report to load
        if let Err(err) = storage.save_report(&report) {
            eprintln!("Skipping load_report benchmark (save_report failed): {err}");
            b.iter(|| ());
            return;
        }
        let report_id = report.id;

        b.iter(|| {
            let _ = black_box(storage.load_report(report_id));
        });
    });

    // Benchmark list_reports
    group.bench_function("list_reports_10", |b| {
        let dir = match tempdir() {
            Ok(dir) => dir,
            Err(err) => {
                eprintln!("Skipping list_reports benchmark (tempdir failed): {err}");
                b.iter(|| ());
                return;
            }
        };
        let storage = IntrospectionStorage::at_path(dir.path().join("introspection"));
        if let Err(err) = storage.initialize() {
            eprintln!("Skipping list_reports benchmark (init failed): {err}");
            b.iter(|| ());
            return;
        }

        // Save 10 reports
        for _ in 0..10 {
            let mut r = report.clone();
            r.id = uuid::Uuid::new_v4();
            if let Err(err) = storage.save_report(&r) {
                eprintln!("Skipping list_reports benchmark (save_report failed): {err}");
                b.iter(|| ());
                return;
            }
        }

        b.iter(|| {
            let _ = black_box(storage.list_reports());
        });
    });

    group.finish();
}

/// Benchmark plan operations
fn bench_plan_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("plan_operations");

    // Benchmark plan creation with builder pattern
    group.bench_function("plan_creation_simple", |b| {
        b.iter(|| {
            black_box(
                ExecutionPlan::new("Test Plan", PlanCategory::Optimization)
                    .with_description("A simple test plan")
                    .with_priority(1),
            );
        });
    });

    group.bench_function("plan_creation_complex", |b| {
        b.iter(|| {
            let mut plan = ExecutionPlan::new("Complex Plan", PlanCategory::ApplicationImprovement)
                .with_description("A complex test plan with many components")
                .with_priority(1)
                .with_steps(vec![
                    ImplementationStep::new(1, "Analyze codebase")
                        .with_files(vec!["src/lib.rs".to_string(), "src/main.rs".to_string()])
                        .with_verification("cargo check"),
                    ImplementationStep::new(2, "Implement feature")
                        .with_files(vec!["src/feature.rs".to_string()])
                        .with_verification("cargo test"),
                    ImplementationStep::new(3, "Update documentation")
                        .with_files(vec!["README.md".to_string()]),
                ])
                .with_success_criteria(vec![
                    "All tests pass".to_string(),
                    "No new warnings".to_string(),
                    "Documentation updated".to_string(),
                ]);
            plan.citations.push(Citation::trace("thread-123"));
            plan.citations
                .push(Citation::commit("abc123", "Related commit"));
            black_box(plan)
        });
    });

    // Benchmark plan validation
    group.bench_function("plan_validation", |b| {
        let plan = ExecutionPlan::new("Test Plan", PlanCategory::Optimization)
            .with_description("Test plan")
            .with_priority(1)
            .with_steps(vec![ImplementationStep::new(1, "Step 1")]);

        b.iter(|| {
            let p = plan.clone();
            black_box(p.validated(0.85));
        });
    });

    // Benchmark plan markdown generation
    let complex_plan = ExecutionPlan::new("Complex Plan", PlanCategory::ApplicationImprovement)
        .with_description("A complex test plan")
        .with_priority(1)
        .with_steps(vec![
            ImplementationStep::new(1, "Step 1").with_files(vec!["file1.rs".to_string()]),
            ImplementationStep::new(2, "Step 2").with_files(vec!["file2.rs".to_string()]),
            ImplementationStep::new(3, "Step 3").with_files(vec!["file3.rs".to_string()]),
        ])
        .with_success_criteria(vec!["Criterion 1".to_string(), "Criterion 2".to_string()]);

    group.bench_function("plan_to_markdown", |b| {
        b.iter(|| {
            black_box(complex_plan.to_markdown());
        });
    });

    group.finish();
}

/// Benchmark consensus building with mock reviewers
fn bench_consensus_building(c: &mut Criterion) {
    let Ok(rt) = tokio::runtime::Runtime::new() else {
        eprintln!("Skipping consensus_building benchmarks (failed to create Tokio runtime)");
        return;
    };
    let mut group = c.benchmark_group("consensus_building");
    group.sample_size(30); // Fewer samples due to async overhead

    // Benchmark consensus with different numbers of reviewers
    for num_reviewers in [1, 3, 5].iter() {
        group.bench_with_input(
            BenchmarkId::new("build_consensus", format!("{}_reviewers", num_reviewers)),
            num_reviewers,
            |b, &num| {
                b.to_async(&rt).iter(|| async {
                    let mut builder = ConsensusBuilder::new().with_min_reviews(num);

                    for i in 0..num {
                        if i % 3 == 0 {
                            builder = builder.add_reviewer(Box::new(MockReviewer::disagreeing(
                                &format!("model-{}", i),
                            )));
                        } else {
                            builder = builder.add_reviewer(Box::new(MockReviewer::agreeing(
                                &format!("model-{}", i),
                            )));
                        }
                    }

                    let proposal = ImprovementProposal {
                        id: uuid::Uuid::new_v4(),
                        title: "Test Proposal".to_string(),
                        description: "Test description".to_string(),
                        source: ProposalSource::Manual,
                        initial_confidence: 0.8,
                        evidence: vec![],
                    };
                    if let Ok(consensus) = builder.build_consensus(&[proposal]).await {
                        black_box(consensus);
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark citation creation
fn bench_citations(c: &mut Criterion) {
    let mut group = c.benchmark_group("citations");

    group.bench_function("citation_trace", |b| {
        b.iter(|| black_box(Citation::trace("thread-12345678")));
    });

    group.bench_function("citation_commit", |b| {
        b.iter(|| black_box(Citation::commit("abc123def456", "Fix bug in parser")));
    });

    group.bench_function("citation_report", |b| {
        let report_id = uuid::Uuid::new_v4();
        b.iter(|| black_box(Citation::report(report_id)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_report_serialization,
    bench_storage_operations,
    bench_plan_operations,
    bench_consensus_building,
    bench_citations,
);
criterion_main!(benches);
