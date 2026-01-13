// `cargo verify` runs clippy with `-D warnings` for all targets, including tests.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use dashflow_evals::golden_dataset::GoldenDataset;

#[test]
#[ignore = "requires fixtures in examples/apps/librarian/data/golden_dataset"]
fn test_load_librarian_golden_dataset() {
    // This test requires the actual golden dataset files to exist
    // Note: librarian uses eval_suite.json format, not golden_dataset directory
    let dataset_path = "examples/apps/librarian/data/golden_dataset";

    assert!(
        std::path::Path::new(dataset_path).exists(),
        "Golden dataset fixtures missing at {}",
        dataset_path
    );

    let dataset =
        GoldenDataset::load(dataset_path).expect("Failed to load librarian golden dataset");

    assert!(dataset.len() >= 3, "Expected at least 3 scenarios");

    // Verify scenario IDs are sorted
    for i in 1..dataset.scenarios.len() {
        assert!(
            dataset.scenarios[i - 1].id <= dataset.scenarios[i].id,
            "Scenarios should be sorted by ID"
        );
    }

    // Verify all scenarios have required fields
    for scenario in &dataset.scenarios {
        assert!(!scenario.id.is_empty());
        assert!(!scenario.description.is_empty());
        assert!(!scenario.query.is_empty());
        assert!(scenario.quality_threshold > 0.0);
        assert!(scenario.quality_threshold <= 1.0);
    }
}

#[test]
#[ignore = "requires fixtures in examples/apps/librarian/data/golden_dataset"]
fn test_filter_and_get_by_id() {
    let dataset_path = "examples/apps/librarian/data/golden_dataset";

    assert!(
        std::path::Path::new(dataset_path).exists(),
        "Golden dataset fixtures missing at {}",
        dataset_path
    );

    let dataset = GoldenDataset::load(dataset_path).expect("Failed to load dataset");

    // Test get_by_id
    if let Some(scenario) = dataset.scenarios.first() {
        let found = dataset.get_by_id(&scenario.id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, scenario.id);
    }

    // Test non-existent ID
    assert!(dataset.get_by_id("nonexistent_id").is_none());
}
