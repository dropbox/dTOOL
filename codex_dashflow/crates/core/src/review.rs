//! Code review types and formatting utilities
//!
//! This module provides types for representing structured code review output
//! and utilities for formatting review findings for display.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Inclusive line range in a file associated with a review finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ReviewLineRange {
    /// Start line (1-indexed, inclusive)
    pub start: u32,
    /// End line (1-indexed, inclusive)
    pub end: u32,
}

/// Location of the code related to a review finding.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ReviewCodeLocation {
    /// Absolute path to the file
    pub absolute_file_path: PathBuf,
    /// Line range within the file
    pub line_range: ReviewLineRange,
}

/// A single review finding describing an observed issue or recommendation.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ReviewFinding {
    /// Short title summarizing the finding
    pub title: String,
    /// Detailed explanation of the issue/recommendation
    pub body: String,
    /// Confidence score (0.0 to 1.0) in the finding
    pub confidence_score: f32,
    /// Priority (higher = more important)
    pub priority: i32,
    /// Location of the code related to this finding
    pub code_location: ReviewCodeLocation,
}

/// Structured review result produced by a review session.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ReviewOutputEvent {
    /// List of individual findings
    pub findings: Vec<ReviewFinding>,
    /// Overall correctness assessment (e.g., "correct", "needs-work", "incorrect")
    pub overall_correctness: String,
    /// Overall explanation of the review
    pub overall_explanation: String,
    /// Overall confidence score (0.0 to 1.0)
    pub overall_confidence_score: f32,
}

impl Default for ReviewOutputEvent {
    fn default() -> Self {
        Self {
            findings: Vec::new(),
            overall_correctness: String::new(),
            overall_explanation: String::new(),
            overall_confidence_score: 0.0,
        }
    }
}

// ============================================================================
// Formatting utilities
// ============================================================================

const REVIEW_FALLBACK_MESSAGE: &str = "Reviewer failed to output a response.";

// ============================================================================
// Review target and prompt generation
// ============================================================================

/// Target for a code review.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReviewTarget {
    /// Review uncommitted changes (staged, unstaged, untracked)
    #[default]
    UncommittedChanges,
    /// Review changes against a base branch
    BaseBranch {
        /// The branch to compare against
        branch: String,
    },
    /// Review a specific commit
    Commit {
        /// The commit SHA to review
        sha: String,
        /// Optional commit title
        title: Option<String>,
    },
    /// Custom review with user-provided instructions
    Custom {
        /// User's review instructions
        instructions: String,
    },
}

/// A resolved review request with generated prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedReviewRequest {
    /// The review target
    pub target: ReviewTarget,
    /// The generated prompt for the LLM
    pub prompt: String,
    /// User-facing hint describing what's being reviewed
    pub user_facing_hint: String,
}

const UNCOMMITTED_PROMPT: &str = "Review the current code changes (staged, unstaged, and untracked files) and provide prioritized findings.";

const BASE_BRANCH_PROMPT_BACKUP: &str = "Review the code changes against the base branch '{branch}'. Start by finding the merge diff between the current branch and {branch}'s upstream e.g. (`git merge-base HEAD \"$(git rev-parse --abbrev-ref \"{branch}@{upstream}\")\"`), then run `git diff` against that SHA to see what changes we would merge into the {branch} branch. Provide prioritized, actionable findings.";
const BASE_BRANCH_PROMPT: &str = "Review the code changes against the base branch '{baseBranch}'. The merge base commit for this comparison is {mergeBaseSha}. Run `git diff {mergeBaseSha}` to inspect the changes relative to {baseBranch}. Provide prioritized, actionable findings.";

const COMMIT_PROMPT_WITH_TITLE: &str = "Review the code changes introduced by commit {sha} (\"{title}\"). Provide prioritized, actionable findings.";
const COMMIT_PROMPT: &str =
    "Review the code changes introduced by commit {sha}. Provide prioritized, actionable findings.";

/// Generate a review prompt for the given target.
///
/// For `BaseBranch` targets, attempts to find the merge-base commit using git.
/// If `merge_base_sha` is provided, it will be used; otherwise `None` triggers
/// a fallback prompt that instructs the reviewer to find the merge-base.
pub fn generate_review_prompt(
    target: &ReviewTarget,
    merge_base_sha: Option<&str>,
) -> Result<String, String> {
    match target {
        ReviewTarget::UncommittedChanges => Ok(UNCOMMITTED_PROMPT.to_string()),
        ReviewTarget::BaseBranch { branch } => {
            if let Some(sha) = merge_base_sha {
                Ok(BASE_BRANCH_PROMPT
                    .replace("{baseBranch}", branch)
                    .replace("{mergeBaseSha}", sha))
            } else {
                Ok(BASE_BRANCH_PROMPT_BACKUP.replace("{branch}", branch))
            }
        }
        ReviewTarget::Commit { sha, title } => {
            if let Some(title) = title {
                Ok(COMMIT_PROMPT_WITH_TITLE
                    .replace("{sha}", sha)
                    .replace("{title}", title))
            } else {
                Ok(COMMIT_PROMPT.replace("{sha}", sha))
            }
        }
        ReviewTarget::Custom { instructions } => {
            let prompt = instructions.trim();
            if prompt.is_empty() {
                Err("Review prompt cannot be empty".to_string())
            } else {
                Ok(prompt.to_string())
            }
        }
    }
}

/// Generate a user-facing hint for the review target.
pub fn review_target_hint(target: &ReviewTarget) -> String {
    match target {
        ReviewTarget::UncommittedChanges => "current changes".to_string(),
        ReviewTarget::BaseBranch { branch } => format!("changes against '{branch}'"),
        ReviewTarget::Commit { sha, title } => {
            let short_sha: String = sha.chars().take(7).collect();
            if let Some(title) = title {
                format!("commit {short_sha}: {title}")
            } else {
                format!("commit {short_sha}")
            }
        }
        ReviewTarget::Custom { instructions } => instructions.trim().to_string(),
    }
}

/// Format a code location as "path:start-end"
fn format_location(item: &ReviewFinding) -> String {
    let path = item.code_location.absolute_file_path.display();
    let start = item.code_location.line_range.start;
    let end = item.code_location.line_range.end;
    format!("{path}:{start}-{end}")
}

/// Format a full review findings block as plain text lines.
///
/// - When `selection` is `Some`, each item line includes a checkbox marker:
///   `[x]` for selected items and `[ ]` for unselected. Missing indices
///   default to selected.
/// - When `selection` is `None`, the marker is omitted and a simple bullet is
///   rendered ("- Title — path:start-end").
pub fn format_review_findings_block(
    findings: &[ReviewFinding],
    selection: Option<&[bool]>,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(String::new());

    // Header
    if findings.len() > 1 {
        lines.push("Full review comments:".to_string());
    } else {
        lines.push("Review comment:".to_string());
    }

    for (idx, item) in findings.iter().enumerate() {
        lines.push(String::new());

        let title = &item.title;
        let location = format_location(item);

        if let Some(flags) = selection {
            // Default to selected if index is out of bounds.
            let checked = flags.get(idx).copied().unwrap_or(true);
            let marker = if checked { "[x]" } else { "[ ]" };
            lines.push(format!("- {marker} {title} — {location}"));
        } else {
            lines.push(format!("- {title} — {location}"));
        }

        for body_line in item.body.lines() {
            lines.push(format!("  {body_line}"));
        }
    }

    lines.join("\n")
}

/// Render a human-readable review summary suitable for a user-facing message.
///
/// Returns either the explanation, the formatted findings block, or both
/// separated by a blank line. If neither is present, emits a fallback message.
pub fn render_review_output_text(output: &ReviewOutputEvent) -> String {
    let mut sections = Vec::new();
    let explanation = output.overall_explanation.trim();
    if !explanation.is_empty() {
        sections.push(explanation.to_string());
    }
    if !output.findings.is_empty() {
        let findings = format_review_findings_block(&output.findings, None);
        let trimmed = findings.trim();
        if !trimmed.is_empty() {
            sections.push(trimmed.to_string());
        }
    }
    if sections.is_empty() {
        REVIEW_FALLBACK_MESSAGE.to_string()
    } else {
        sections.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_finding(title: &str, body: &str, path: &str, start: u32, end: u32) -> ReviewFinding {
        ReviewFinding {
            title: title.to_string(),
            body: body.to_string(),
            confidence_score: 0.8,
            priority: 1,
            code_location: ReviewCodeLocation {
                absolute_file_path: PathBuf::from(path),
                line_range: ReviewLineRange { start, end },
            },
        }
    }

    #[test]
    fn test_review_line_range_serialization() {
        let range = ReviewLineRange { start: 10, end: 20 };
        let json = serde_json::to_string(&range).unwrap();
        let parsed: ReviewLineRange = serde_json::from_str(&json).unwrap();
        assert_eq!(range, parsed);
    }

    #[test]
    fn test_review_code_location_serialization() {
        let loc = ReviewCodeLocation {
            absolute_file_path: PathBuf::from("/src/main.rs"),
            line_range: ReviewLineRange { start: 1, end: 10 },
        };
        let json = serde_json::to_string(&loc).unwrap();
        let parsed: ReviewCodeLocation = serde_json::from_str(&json).unwrap();
        assert_eq!(loc, parsed);
    }

    #[test]
    fn test_review_finding_serialization() {
        let finding = make_finding("Bug", "Found a bug", "/src/lib.rs", 5, 15);
        let json = serde_json::to_string(&finding).unwrap();
        let parsed: ReviewFinding = serde_json::from_str(&json).unwrap();
        assert_eq!(finding, parsed);
    }

    #[test]
    fn test_review_output_event_default() {
        let output = ReviewOutputEvent::default();
        assert!(output.findings.is_empty());
        assert!(output.overall_explanation.is_empty());
        assert_eq!(output.overall_confidence_score, 0.0);
    }

    #[test]
    fn test_format_location() {
        let finding = make_finding("Test", "Body", "/path/to/file.rs", 10, 20);
        let location = format_location(&finding);
        assert_eq!(location, "/path/to/file.rs:10-20");
    }

    #[test]
    fn test_format_review_findings_single() {
        let findings = vec![make_finding(
            "Missing return",
            "Consider adding return",
            "/src/main.rs",
            5,
            5,
        )];
        let text = format_review_findings_block(&findings, None);
        assert!(text.contains("Review comment:"));
        assert!(text.contains("Missing return"));
        assert!(text.contains("/src/main.rs:5-5"));
        assert!(text.contains("Consider adding return"));
    }

    #[test]
    fn test_format_review_findings_multiple() {
        let findings = vec![
            make_finding("Bug 1", "First bug", "/src/a.rs", 1, 5),
            make_finding("Bug 2", "Second bug", "/src/b.rs", 10, 20),
        ];
        let text = format_review_findings_block(&findings, None);
        assert!(text.contains("Full review comments:"));
        assert!(text.contains("Bug 1"));
        assert!(text.contains("Bug 2"));
    }

    #[test]
    fn test_format_review_findings_with_selection() {
        let findings = vec![
            make_finding("Bug 1", "First", "/src/a.rs", 1, 5),
            make_finding("Bug 2", "Second", "/src/b.rs", 10, 20),
        ];
        let selection = vec![true, false];
        let text = format_review_findings_block(&findings, Some(&selection));
        assert!(text.contains("[x] Bug 1"));
        assert!(text.contains("[ ] Bug 2"));
    }

    #[test]
    fn test_format_review_findings_selection_defaults_to_checked() {
        let findings = vec![
            make_finding("Bug 1", "First", "/src/a.rs", 1, 5),
            make_finding("Bug 2", "Second", "/src/b.rs", 10, 20),
            make_finding("Bug 3", "Third", "/src/c.rs", 30, 40),
        ];
        // Only first item specified as unchecked, others should default to checked
        let selection = vec![false];
        let text = format_review_findings_block(&findings, Some(&selection));
        assert!(text.contains("[ ] Bug 1"));
        assert!(text.contains("[x] Bug 2"));
        assert!(text.contains("[x] Bug 3"));
    }

    #[test]
    fn test_format_review_findings_multiline_body() {
        let findings = vec![make_finding(
            "Bug",
            "Line 1\nLine 2\nLine 3",
            "/src/main.rs",
            1,
            5,
        )];
        let text = format_review_findings_block(&findings, None);
        assert!(text.contains("  Line 1"));
        assert!(text.contains("  Line 2"));
        assert!(text.contains("  Line 3"));
    }

    #[test]
    fn test_render_review_output_explanation_only() {
        let output = ReviewOutputEvent {
            findings: vec![],
            overall_correctness: "correct".to_string(),
            overall_explanation: "Code looks good.".to_string(),
            overall_confidence_score: 0.9,
        };
        let text = render_review_output_text(&output);
        assert_eq!(text, "Code looks good.");
    }

    #[test]
    fn test_render_review_output_findings_only() {
        let output = ReviewOutputEvent {
            findings: vec![make_finding("Issue", "Detail", "/src/main.rs", 1, 5)],
            overall_correctness: String::new(),
            overall_explanation: String::new(),
            overall_confidence_score: 0.0,
        };
        let text = render_review_output_text(&output);
        assert!(text.contains("Review comment:"));
        assert!(text.contains("Issue"));
    }

    #[test]
    fn test_render_review_output_both() {
        let output = ReviewOutputEvent {
            findings: vec![make_finding("Issue", "Detail", "/src/main.rs", 1, 5)],
            overall_correctness: "needs-work".to_string(),
            overall_explanation: "Some issues found.".to_string(),
            overall_confidence_score: 0.7,
        };
        let text = render_review_output_text(&output);
        assert!(text.contains("Some issues found."));
        assert!(text.contains("Review comment:"));
        assert!(text.contains("\n\n")); // Sections separated by blank line
    }

    #[test]
    fn test_render_review_output_empty() {
        let output = ReviewOutputEvent::default();
        let text = render_review_output_text(&output);
        assert_eq!(text, "Reviewer failed to output a response.");
    }

    #[test]
    fn test_render_review_output_whitespace_only_explanation() {
        let output = ReviewOutputEvent {
            findings: vec![],
            overall_correctness: String::new(),
            overall_explanation: "   \n\t  ".to_string(),
            overall_confidence_score: 0.0,
        };
        let text = render_review_output_text(&output);
        assert_eq!(text, "Reviewer failed to output a response.");
    }

    // ========================================================================
    // Review target and prompt tests
    // ========================================================================

    #[test]
    fn test_review_target_default() {
        let target = ReviewTarget::default();
        assert_eq!(target, ReviewTarget::UncommittedChanges);
    }

    #[test]
    fn test_review_target_serialization_uncommitted() {
        let target = ReviewTarget::UncommittedChanges;
        let json = serde_json::to_string(&target).unwrap();
        let parsed: ReviewTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
    }

    #[test]
    fn test_review_target_serialization_base_branch() {
        let target = ReviewTarget::BaseBranch {
            branch: "main".to_string(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let parsed: ReviewTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
    }

    #[test]
    fn test_review_target_serialization_commit() {
        let target = ReviewTarget::Commit {
            sha: "abc123".to_string(),
            title: Some("Fix bug".to_string()),
        };
        let json = serde_json::to_string(&target).unwrap();
        let parsed: ReviewTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
    }

    #[test]
    fn test_review_target_serialization_custom() {
        let target = ReviewTarget::Custom {
            instructions: "Review for security".to_string(),
        };
        let json = serde_json::to_string(&target).unwrap();
        let parsed: ReviewTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(target, parsed);
    }

    #[test]
    fn test_generate_review_prompt_uncommitted() {
        let target = ReviewTarget::UncommittedChanges;
        let prompt = generate_review_prompt(&target, None).unwrap();
        assert!(prompt.contains("current code changes"));
        assert!(prompt.contains("staged, unstaged, and untracked"));
    }

    #[test]
    fn test_generate_review_prompt_base_branch_with_sha() {
        let target = ReviewTarget::BaseBranch {
            branch: "main".to_string(),
        };
        let prompt = generate_review_prompt(&target, Some("abc123def456")).unwrap();
        assert!(prompt.contains("main"));
        assert!(prompt.contains("abc123def456"));
        assert!(prompt.contains("git diff abc123def456"));
    }

    #[test]
    fn test_generate_review_prompt_base_branch_without_sha() {
        let target = ReviewTarget::BaseBranch {
            branch: "develop".to_string(),
        };
        let prompt = generate_review_prompt(&target, None).unwrap();
        assert!(prompt.contains("develop"));
        assert!(prompt.contains("merge-base"));
        assert!(prompt.contains("upstream"));
    }

    #[test]
    fn test_generate_review_prompt_commit_with_title() {
        let target = ReviewTarget::Commit {
            sha: "abc123".to_string(),
            title: Some("Fix critical bug".to_string()),
        };
        let prompt = generate_review_prompt(&target, None).unwrap();
        assert!(prompt.contains("abc123"));
        assert!(prompt.contains("Fix critical bug"));
    }

    #[test]
    fn test_generate_review_prompt_commit_without_title() {
        let target = ReviewTarget::Commit {
            sha: "def456".to_string(),
            title: None,
        };
        let prompt = generate_review_prompt(&target, None).unwrap();
        assert!(prompt.contains("def456"));
        assert!(!prompt.contains("(\""));
    }

    #[test]
    fn test_generate_review_prompt_custom() {
        let target = ReviewTarget::Custom {
            instructions: "Focus on error handling".to_string(),
        };
        let prompt = generate_review_prompt(&target, None).unwrap();
        assert_eq!(prompt, "Focus on error handling");
    }

    #[test]
    fn test_generate_review_prompt_custom_empty() {
        let target = ReviewTarget::Custom {
            instructions: "   ".to_string(),
        };
        let result = generate_review_prompt(&target, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("empty"));
    }

    #[test]
    fn test_review_target_hint_uncommitted() {
        let target = ReviewTarget::UncommittedChanges;
        let hint = review_target_hint(&target);
        assert_eq!(hint, "current changes");
    }

    #[test]
    fn test_review_target_hint_base_branch() {
        let target = ReviewTarget::BaseBranch {
            branch: "main".to_string(),
        };
        let hint = review_target_hint(&target);
        assert_eq!(hint, "changes against 'main'");
    }

    #[test]
    fn test_review_target_hint_commit_with_title() {
        let target = ReviewTarget::Commit {
            sha: "abc123456789".to_string(),
            title: Some("Fix bug".to_string()),
        };
        let hint = review_target_hint(&target);
        assert_eq!(hint, "commit abc1234: Fix bug");
    }

    #[test]
    fn test_review_target_hint_commit_without_title() {
        let target = ReviewTarget::Commit {
            sha: "abc123456789".to_string(),
            title: None,
        };
        let hint = review_target_hint(&target);
        assert_eq!(hint, "commit abc1234");
    }

    #[test]
    fn test_review_target_hint_custom() {
        let target = ReviewTarget::Custom {
            instructions: "  Security review  ".to_string(),
        };
        let hint = review_target_hint(&target);
        assert_eq!(hint, "Security review");
    }
}
