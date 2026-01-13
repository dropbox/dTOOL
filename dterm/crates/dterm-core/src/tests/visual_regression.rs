//! Visual regression tests for GPU rendering.
//!
//! These tests verify that special characters (box drawing, block elements,
//! powerline glyphs) are rendered correctly and produce visible output.
//!
//! # How to Run
//!
//! ```bash
//! cargo test --package dterm-core --features visual-testing -- visual_regression
//! ```
//!
//! # Generating Golden Images
//!
//! Set `UPDATE_GOLDEN=1` environment variable to generate new golden images:
//!
//! ```bash
//! UPDATE_GOLDEN=1 cargo test --package dterm-core --features visual-testing -- visual_regression
//! ```
//!
//! # Test Structure
//!
//! 1. **Non-empty output tests**: Verify that rendering produces any visible output
//! 2. **Character coverage tests**: Verify specific Unicode ranges generate vertices
//! 3. **Golden comparison tests**: Compare rendered output against reference images
//!
//! Note: Golden comparison tests require a GPU and may be skipped in CI without
//! graphics capability.

#[cfg(feature = "visual-testing")]
mod gpu_tests {
    use crate::gpu::visual_testing::{CompareConfig, VisualTestHarness};
    use crate::terminal::Terminal;
    use std::path::PathBuf;

    /// Helper to create a terminal with specific content.
    fn create_terminal_with(content: &str, cols: u16, rows: u16) -> Terminal {
        let mut terminal = Terminal::new(cols, rows);
        for c in content.chars() {
            if c == '\n' {
                // Move to next line
                terminal.process(b"\r\n");
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                terminal.process(s.as_bytes());
            }
        }
        terminal
    }

    /// Helper to check if we can create a GPU context.
    async fn can_create_gpu() -> bool {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .is_some()
    }

    /// Get the golden directory path.
    fn golden_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
    }

    /// Check if we should update golden images (UPDATE_GOLDEN=1).
    fn should_update_golden() -> bool {
        std::env::var("UPDATE_GOLDEN")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
    }

    // ============================================================
    // Box Drawing Tests
    // ============================================================

    #[tokio::test]
    async fn test_box_drawing_3x3_produces_output() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        let content = "┌─┐\n│X│\n└─┘";
        let terminal = create_terminal_with(content, 3, 3);

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness");

        // Verify rendering produces non-empty output
        let has_content = harness
            .verify_non_empty(&terminal, 24, 48) // 3 cols * 8px, 3 rows * 16px
            .await
            .expect("Failed to render");

        assert!(
            has_content,
            "Box drawing 3x3 should produce visible output"
        );
    }

    #[tokio::test]
    async fn test_box_drawing_comprehensive_produces_output() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        // Test a representative sample of box drawing characters
        let sample = "─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬";
        let terminal = create_terminal_with(sample, 22, 1);

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness");

        let has_content = harness
            .verify_non_empty(&terminal, 176, 16) // 22 cols * 8px, 1 row * 16px
            .await
            .expect("Failed to render");

        assert!(
            has_content,
            "Box drawing sample should produce visible output"
        );
    }

    // ============================================================
    // Block Element Tests
    // ============================================================

    #[tokio::test]
    async fn test_block_elements_produce_output() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        // Block elements: shades and quadrants
        let sample = "▀▄█░▒▓▐▌";
        let terminal = create_terminal_with(sample, 8, 1);

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness");

        let has_content = harness
            .verify_non_empty(&terminal, 64, 16)
            .await
            .expect("Failed to render");

        assert!(
            has_content,
            "Block elements should produce visible output"
        );
    }

    // ============================================================
    // Powerline Tests
    // ============================================================

    #[tokio::test]
    async fn test_powerline_glyphs_produce_output() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        // Powerline glyphs (U+E0A0-E0D7)
        let sample = "\u{E0A0}\u{E0A1}\u{E0A2}\u{E0B0}\u{E0B1}\u{E0B2}\u{E0B3}";
        let terminal = create_terminal_with(sample, 7, 1);

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness");

        let has_content = harness
            .verify_non_empty(&terminal, 56, 16)
            .await
            .expect("Failed to render");

        assert!(
            has_content,
            "Powerline glyphs should produce visible output"
        );
    }

    // ============================================================
    // Golden Image Comparison Tests
    // ============================================================

    #[tokio::test]
    async fn test_box_drawing_golden() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        let content = "┌─┐\n│X│\n└─┘";
        let terminal = create_terminal_with(content, 3, 3);

        let config = CompareConfig {
            update_golden: should_update_golden(),
            max_diff_percentage: 0.0,
            ..Default::default()
        };

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness")
            .with_config(config);

        let result = harness
            .render_and_compare(&terminal, "box_drawing_3x3")
            .await;

        match result {
            Ok(compare) => {
                assert!(
                    compare.is_match(),
                    "Visual regression: {}\nDiff image: {:?}",
                    compare.description,
                    compare.diff_image_path
                );
            }
            Err(crate::gpu::visual_testing::VisualTestError::GoldenNotFound(_)) => {
                if !should_update_golden() {
                    eprintln!(
                        "Golden image not found. Run with UPDATE_GOLDEN=1 to generate."
                    );
                }
            }
            Err(e) => panic!("Visual test failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_block_elements_golden() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        let sample = "▀▄█░▒▓▐▌";
        let terminal = create_terminal_with(sample, 8, 1);

        let config = CompareConfig {
            update_golden: should_update_golden(),
            max_diff_percentage: 0.0,
            ..Default::default()
        };

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness")
            .with_config(config);

        let result = harness
            .render_and_compare(&terminal, "block_elements")
            .await;

        match result {
            Ok(compare) => {
                assert!(
                    compare.is_match(),
                    "Visual regression: {}\nDiff image: {:?}",
                    compare.description,
                    compare.diff_image_path
                );
            }
            Err(crate::gpu::visual_testing::VisualTestError::GoldenNotFound(_)) => {
                if !should_update_golden() {
                    eprintln!(
                        "Golden image not found. Run with UPDATE_GOLDEN=1 to generate."
                    );
                }
            }
            Err(e) => panic!("Visual test failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_powerline_golden() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        let sample = "\u{E0A0}\u{E0A1}\u{E0A2}\u{E0B0}\u{E0B1}\u{E0B2}\u{E0B3}";
        let terminal = create_terminal_with(sample, 7, 1);

        let config = CompareConfig {
            update_golden: should_update_golden(),
            max_diff_percentage: 0.0,
            ..Default::default()
        };

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness")
            .with_config(config);

        let result = harness
            .render_and_compare(&terminal, "powerline_glyphs")
            .await;

        match result {
            Ok(compare) => {
                assert!(
                    compare.is_match(),
                    "Visual regression: {}\nDiff image: {:?}",
                    compare.description,
                    compare.diff_image_path
                );
            }
            Err(crate::gpu::visual_testing::VisualTestError::GoldenNotFound(_)) => {
                if !should_update_golden() {
                    eprintln!(
                        "Golden image not found. Run with UPDATE_GOLDEN=1 to generate."
                    );
                }
            }
            Err(e) => panic!("Visual test failed: {}", e),
        }
    }

    // ============================================================
    // Mixed Content Tests
    // ============================================================

    #[tokio::test]
    async fn test_mixed_content_produces_output() {
        if !can_create_gpu().await {
            eprintln!("Skipping test: no GPU available");
            return;
        }

        // Mix of ASCII, box drawing, and block elements
        let content = "┌──────┐\n│Hello!│\n│░▒▓▓▒░│\n└──────┘";
        let terminal = create_terminal_with(content, 8, 4);

        let harness = VisualTestHarness::with_golden_dir(golden_dir())
            .await
            .expect("Failed to create test harness");

        let has_content = harness
            .verify_non_empty(&terminal, 64, 64)
            .await
            .expect("Failed to render");

        assert!(
            has_content,
            "Mixed content should produce visible output"
        );
    }
}
