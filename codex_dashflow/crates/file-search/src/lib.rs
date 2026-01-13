//! Fuzzy file search using nucleo-matcher
//!
//! This crate provides fuzzy file searching capabilities similar to
//! what's used in fzf, telescope.nvim, etc. It walks the file system
//! respecting .gitignore rules and scores files against a fuzzy pattern.

use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use nucleo_matcher::pattern::{AtomKind, CaseMatching, Normalization, Pattern};
use nucleo_matcher::Matcher;
use nucleo_matcher::Utf32Str;
use serde::Serialize;
use std::cell::UnsafeCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::num::NonZero;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// A single match result returned from the search.
#[derive(Debug, Clone, Serialize)]
pub struct FileMatch {
    /// Relevance score returned by nucleo_matcher (higher is better)
    pub score: u32,
    /// Path to the matched file (relative to the search directory)
    pub path: String,
    /// Character indices that matched the query (for highlighting)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indices: Option<Vec<u32>>,
}

/// Results from a file search operation
#[derive(Debug)]
pub struct FileSearchResults {
    /// The matched files, sorted by score descending
    pub matches: Vec<FileMatch>,
    /// Total number of files that matched (may be more than matches.len() if limit was applied)
    pub total_match_count: usize,
}

/// Configuration for file search
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of results to return
    pub limit: usize,
    /// Whether to compute match indices for highlighting
    pub compute_indices: bool,
    /// Whether to respect .gitignore files
    pub respect_gitignore: bool,
    /// Glob patterns to exclude (e.g., "*.log", "node_modules/**")
    pub exclude: Vec<String>,
    /// Number of threads to use for parallel file walking
    pub threads: usize,
    /// Audit #59: Whether to respect project-specific ignore files (.codexignore)
    pub respect_codexignore: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            limit: 100,
            compute_indices: false,
            respect_gitignore: true,
            exclude: Vec::new(),
            threads: num_cpus(),
            respect_codexignore: true, // Audit #59: Default to respecting project-specific ignores
        }
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Audit #59: Load patterns from .codexignore file
///
/// The .codexignore file uses the same format as .gitignore:
/// - One pattern per line
/// - Lines starting with # are comments
/// - Empty lines are ignored
/// - Patterns follow gitignore glob syntax
fn load_codexignore(directory: &Path) -> Option<Vec<String>> {
    let codexignore_path = directory.join(".codexignore");
    if !codexignore_path.exists() {
        return None;
    }

    let contents = std::fs::read_to_string(&codexignore_path).ok()?;
    let patterns: Vec<String> = contents
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect();

    if patterns.is_empty() {
        None
    } else {
        Some(patterns)
    }
}

/// Run a fuzzy file search in the given directory.
///
/// # Arguments
/// * `pattern` - The fuzzy search pattern
/// * `directory` - The directory to search in
/// * `config` - Search configuration
/// * `cancel_flag` - Optional flag to cancel the search early
///
/// # Returns
/// A `FileSearchResults` containing the matched files sorted by score.
pub fn search(
    pattern: &str,
    directory: &Path,
    config: &SearchConfig,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> anyhow::Result<FileSearchResults> {
    let cancel = cancel_flag.unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    let limit = NonZero::new(config.limit).unwrap_or(NonZero::new(100).unwrap());
    let threads = NonZero::new(config.threads).unwrap_or(NonZero::new(4).unwrap());

    // Audit #59: Combine explicit excludes with patterns from .codexignore
    let mut all_excludes = config.exclude.clone();
    if config.respect_codexignore {
        if let Some(codexignore_excludes) = load_codexignore(directory) {
            all_excludes.extend(codexignore_excludes);
        }
    }

    run_search(
        pattern,
        limit,
        directory,
        all_excludes,
        threads,
        cancel,
        config.compute_indices,
        config.respect_gitignore,
    )
}

/// Async wrapper for search
pub async fn search_async(
    pattern: &str,
    directory: &Path,
    config: &SearchConfig,
    cancel_flag: Option<Arc<AtomicBool>>,
) -> anyhow::Result<FileSearchResults> {
    let pattern = pattern.to_string();
    let directory = directory.to_path_buf();
    let config = config.clone();

    tokio::task::spawn_blocking(move || search(&pattern, &directory, &config, cancel_flag)).await?
}

#[allow(clippy::too_many_arguments)]
fn run_search(
    pattern_text: &str,
    limit: NonZero<usize>,
    search_directory: &Path,
    exclude: Vec<String>,
    threads: NonZero<usize>,
    cancel_flag: Arc<AtomicBool>,
    compute_indices: bool,
    respect_gitignore: bool,
) -> anyhow::Result<FileSearchResults> {
    let pattern = create_pattern(pattern_text);

    // Create worker count - WalkBuilder calls the builder function threads+1 times
    let num_walk_builder_threads = threads.get();
    let num_best_matches_lists = num_walk_builder_threads + 1;

    // Create one BestMatchesList per worker thread
    let best_matchers_per_worker: Vec<UnsafeCell<BestMatchesList>> = (0..num_best_matches_lists)
        .map(|_| {
            UnsafeCell::new(BestMatchesList::new(
                limit.get(),
                pattern.clone(),
                Matcher::new(nucleo_matcher::Config::DEFAULT),
            ))
        })
        .collect();

    // Use ignore crate's parallel walker (same as ripgrep)
    let mut walk_builder = WalkBuilder::new(search_directory);
    walk_builder
        .threads(num_walk_builder_threads)
        .hidden(false)
        .follow_links(true)
        .require_git(false);

    if !respect_gitignore {
        walk_builder
            .git_ignore(false)
            .git_global(false)
            .git_exclude(false)
            .ignore(false)
            .parents(false);
    }

    if !exclude.is_empty() {
        let mut override_builder = OverrideBuilder::new(search_directory);
        for pattern in exclude {
            let exclude_pattern = format!("!{pattern}");
            override_builder.add(&exclude_pattern)?;
        }
        let override_matcher = override_builder.build()?;
        walk_builder.overrides(override_matcher);
    }

    let walker = walk_builder.build_parallel();

    let index_counter = AtomicUsize::new(0);
    walker.run(|| {
        let index = index_counter.fetch_add(1, Ordering::Relaxed);
        let best_list_ptr = best_matchers_per_worker[index].get();
        let best_list = unsafe { &mut *best_list_ptr };

        const CHECK_INTERVAL: usize = 1024;
        let mut processed = 0;
        let cancel = cancel_flag.clone();

        Box::new(move |entry| {
            if let Some(path) = get_file_path(&entry, search_directory) {
                best_list.insert(path);
            }

            processed += 1;
            if processed % CHECK_INTERVAL == 0 && cancel.load(Ordering::Relaxed) {
                ignore::WalkState::Quit
            } else {
                ignore::WalkState::Continue
            }
        })
    });

    if cancel_flag.load(Ordering::Relaxed) {
        return Ok(FileSearchResults {
            matches: Vec::new(),
            total_match_count: 0,
        });
    }

    // Merge results from all workers
    let mut global_heap: BinaryHeap<Reverse<(u32, String)>> = BinaryHeap::new();
    let mut total_match_count = 0;

    for best_list_cell in best_matchers_per_worker.iter() {
        let best_list = unsafe { &*best_list_cell.get() };
        total_match_count += best_list.num_matches;

        for &Reverse((score, ref path)) in best_list.binary_heap.iter() {
            if global_heap.len() < limit.get() {
                global_heap.push(Reverse((score, path.clone())));
            } else if let Some(min) = global_heap.peek() {
                if score > min.0 .0 {
                    global_heap.pop();
                    global_heap.push(Reverse((score, path.clone())));
                }
            }
        }
    }

    let mut raw_matches: Vec<(u32, String)> = global_heap.into_iter().map(|r| r.0).collect();
    sort_matches(&mut raw_matches);

    // Transform to FileMatch, optionally computing indices
    let mut matcher = if compute_indices {
        Some(Matcher::new(nucleo_matcher::Config::DEFAULT))
    } else {
        None
    };

    let matches: Vec<FileMatch> = raw_matches
        .into_iter()
        .map(|(score, path)| {
            let indices = if compute_indices {
                let mut buf = Vec::<char>::new();
                let haystack: Utf32Str<'_> = Utf32Str::new(&path, &mut buf);
                let mut idx_vec: Vec<u32> = Vec::new();
                if let Some(ref mut m) = matcher {
                    pattern.indices(haystack, m, &mut idx_vec);
                }
                idx_vec.sort_unstable();
                idx_vec.dedup();
                Some(idx_vec)
            } else {
                None
            };

            FileMatch {
                score,
                path,
                indices,
            }
        })
        .collect();

    Ok(FileSearchResults {
        matches,
        total_match_count,
    })
}

fn get_file_path<'a>(
    entry_result: &'a Result<ignore::DirEntry, ignore::Error>,
    search_directory: &Path,
) -> Option<&'a str> {
    let entry = entry_result.as_ref().ok()?;
    if entry.file_type().is_some_and(|ft| ft.is_dir()) {
        return None;
    }
    let path = entry.path();
    path.strip_prefix(search_directory).ok()?.to_str()
}

fn sort_matches(matches: &mut [(u32, String)]) {
    matches.sort_by(|a, b| match b.0.cmp(&a.0) {
        std::cmp::Ordering::Equal => a.1.cmp(&b.1),
        other => other,
    });
}

fn create_pattern(pattern: &str) -> Pattern {
    Pattern::new(
        pattern,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
    )
}

/// Maintains the top N matches for a pattern
struct BestMatchesList {
    max_count: usize,
    num_matches: usize,
    pattern: Pattern,
    matcher: Matcher,
    binary_heap: BinaryHeap<Reverse<(u32, String)>>,
    utf32buf: Vec<char>,
}

impl BestMatchesList {
    fn new(max_count: usize, pattern: Pattern, matcher: Matcher) -> Self {
        Self {
            max_count,
            num_matches: 0,
            pattern,
            matcher,
            binary_heap: BinaryHeap::new(),
            utf32buf: Vec::new(),
        }
    }

    fn insert(&mut self, line: &str) {
        let haystack: Utf32Str<'_> = Utf32Str::new(line, &mut self.utf32buf);
        if let Some(score) = self.pattern.score(haystack, &mut self.matcher) {
            self.num_matches += 1;

            if self.binary_heap.len() < self.max_count {
                self.binary_heap.push(Reverse((score, line.to_string())));
            } else if let Some(min) = self.binary_heap.peek() {
                if score > min.0 .0 {
                    self.binary_heap.pop();
                    self.binary_heap.push(Reverse((score, line.to_string())));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_non_match_returns_none() {
        let mut utf32buf = Vec::new();
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = create_pattern("zzz");
        let haystack: Utf32Str<'_> = Utf32Str::new("hello", &mut utf32buf);
        assert_eq!(pattern.score(haystack, &mut matcher), None);
    }

    #[test]
    fn test_pattern_match_returns_score() {
        let mut utf32buf = Vec::new();
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let pattern = create_pattern("hlo");
        let haystack: Utf32Str<'_> = Utf32Str::new("hello", &mut utf32buf);
        assert!(pattern.score(haystack, &mut matcher).is_some());
    }

    #[test]
    fn test_sort_by_score_desc() {
        let mut matches = vec![
            (100, "b.rs".to_string()),
            (200, "a.rs".to_string()),
            (100, "a.rs".to_string()),
        ];
        sort_matches(&mut matches);

        assert_eq!(matches[0], (200, "a.rs".to_string()));
        assert_eq!(matches[1], (100, "a.rs".to_string()));
        assert_eq!(matches[2], (100, "b.rs".to_string()));
    }

    #[test]
    fn test_search_current_directory() {
        let config = SearchConfig {
            limit: 10,
            ..Default::default()
        };

        // Search for Cargo.toml in current directory
        let result = search("Cargo", Path::new("."), &config, None);
        assert!(result.is_ok());

        let results = result.unwrap();
        // Should find at least one Cargo.toml
        assert!(results.total_match_count > 0);
    }

    #[test]
    fn test_search_with_exclusion() {
        let config = SearchConfig {
            limit: 100,
            exclude: vec!["target/**".to_string()],
            ..Default::default()
        };

        let result = search("rs", Path::new("."), &config, None);
        assert!(result.is_ok());

        let results = result.unwrap();
        // Should not include files from target directory
        for m in &results.matches {
            assert!(!m.path.starts_with("target/"));
        }
    }

    #[test]
    fn test_search_config_default() {
        let config = SearchConfig::default();
        assert_eq!(config.limit, 100);
        assert!(!config.compute_indices);
        assert!(config.respect_gitignore);
        assert!(config.exclude.is_empty());
        assert!(config.threads > 0);
    }

    #[test]
    fn test_search_config_custom() {
        let config = SearchConfig {
            limit: 50,
            compute_indices: true,
            respect_gitignore: false,
            exclude: vec!["*.log".to_string()],
            threads: 2,
            respect_codexignore: false,
        };
        assert_eq!(config.limit, 50);
        assert!(config.compute_indices);
        assert!(!config.respect_gitignore);
        assert_eq!(config.exclude.len(), 1);
        assert_eq!(config.threads, 2);
        assert!(!config.respect_codexignore);
    }

    #[test]
    fn test_file_match_struct() {
        let file_match = FileMatch {
            score: 100,
            path: "src/main.rs".to_string(),
            indices: Some(vec![0, 4, 5]),
        };
        assert_eq!(file_match.score, 100);
        assert_eq!(file_match.path, "src/main.rs");
        assert_eq!(file_match.indices, Some(vec![0, 4, 5]));
    }

    #[test]
    fn test_file_match_no_indices() {
        let file_match = FileMatch {
            score: 50,
            path: "test.txt".to_string(),
            indices: None,
        };
        assert!(file_match.indices.is_none());
    }

    #[test]
    fn test_file_match_serialization() {
        let file_match = FileMatch {
            score: 75,
            path: "path/to/file.rs".to_string(),
            indices: Some(vec![1, 2, 3]),
        };
        let json = serde_json::to_string(&file_match).unwrap();
        assert!(json.contains("\"score\":75"));
        assert!(json.contains("path/to/file.rs"));
        assert!(json.contains("[1,2,3]"));
    }

    #[test]
    fn test_file_match_serialization_skip_none_indices() {
        let file_match = FileMatch {
            score: 50,
            path: "file.rs".to_string(),
            indices: None,
        };
        let json = serde_json::to_string(&file_match).unwrap();
        assert!(!json.contains("indices"));
    }

    #[test]
    fn test_file_search_results_struct() {
        let results = FileSearchResults {
            matches: vec![
                FileMatch {
                    score: 100,
                    path: "a.rs".to_string(),
                    indices: None,
                },
                FileMatch {
                    score: 50,
                    path: "b.rs".to_string(),
                    indices: None,
                },
            ],
            total_match_count: 10,
        };
        assert_eq!(results.matches.len(), 2);
        assert_eq!(results.total_match_count, 10);
    }

    #[test]
    fn test_search_empty_pattern() {
        let config = SearchConfig {
            limit: 10,
            ..Default::default()
        };

        // Empty pattern should still work
        let result = search("", Path::new("."), &config, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_with_compute_indices() {
        let config = SearchConfig {
            limit: 10,
            compute_indices: true,
            ..Default::default()
        };

        let result = search("Cargo", Path::new("."), &config, None);
        assert!(result.is_ok());

        let results = result.unwrap();
        if !results.matches.is_empty() {
            // With compute_indices true, indices should be populated
            assert!(results.matches[0].indices.is_some());
        }
    }

    #[test]
    fn test_search_with_no_gitignore() {
        let config = SearchConfig {
            limit: 10,
            respect_gitignore: false,
            ..Default::default()
        };

        let result = search("rs", Path::new("."), &config, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_with_cancel_flag() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let cancel = Arc::new(AtomicBool::new(true)); // Already cancelled
        let config = SearchConfig::default();

        let result = search("test", Path::new("."), &config, Some(cancel));
        assert!(result.is_ok());

        let results = result.unwrap();
        // When cancelled from the start, should return empty
        assert!(results.matches.is_empty());
        assert_eq!(results.total_match_count, 0);
    }

    #[test]
    fn test_search_limit_respected() {
        let config = SearchConfig {
            limit: 3,
            ..Default::default()
        };

        let result = search("rs", Path::new("."), &config, None);
        assert!(result.is_ok());

        let results = result.unwrap();
        assert!(results.matches.len() <= 3);
    }

    #[test]
    fn test_search_multiple_exclusions() {
        let config = SearchConfig {
            limit: 50,
            exclude: vec![
                "target/**".to_string(),
                "*.lock".to_string(),
                "node_modules/**".to_string(),
            ],
            ..Default::default()
        };

        let result = search("rs", Path::new("."), &config, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_sort_matches_by_score() {
        let mut matches = vec![
            (50, "low.rs".to_string()),
            (100, "high.rs".to_string()),
            (75, "mid.rs".to_string()),
        ];
        sort_matches(&mut matches);

        assert_eq!(matches[0].0, 100);
        assert_eq!(matches[1].0, 75);
        assert_eq!(matches[2].0, 50);
    }

    #[test]
    fn test_sort_matches_alphabetically_on_tie() {
        let mut matches = vec![
            (100, "b.rs".to_string()),
            (100, "a.rs".to_string()),
            (100, "c.rs".to_string()),
        ];
        sort_matches(&mut matches);

        assert_eq!(matches[0].1, "a.rs");
        assert_eq!(matches[1].1, "b.rs");
        assert_eq!(matches[2].1, "c.rs");
    }

    #[test]
    fn test_create_pattern() {
        let pattern = create_pattern("test");
        // Pattern should be created without panic
        let mut utf32buf = Vec::new();
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let haystack: Utf32Str<'_> = Utf32Str::new("testing", &mut utf32buf);
        assert!(pattern.score(haystack, &mut matcher).is_some());
    }

    #[test]
    fn test_best_matches_list_insert_below_limit() {
        let pattern = create_pattern("test");
        let matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let mut list = BestMatchesList::new(5, pattern, matcher);

        list.insert("test1.rs");
        list.insert("test2.rs");

        assert!(list.binary_heap.len() <= 5);
        // num_matches is usize and is properly initialized
        assert_eq!(list.num_matches, 2);
    }

    #[test]
    fn test_best_matches_list_insert_exceeds_limit() {
        let pattern = create_pattern("rs");
        let matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let mut list = BestMatchesList::new(2, pattern, matcher);

        list.insert("a.rs");
        list.insert("b.rs");
        list.insert("c.rs");
        list.insert("d.rs");

        // Should keep only top 2
        assert!(list.binary_heap.len() <= 2);
    }

    #[test]
    fn test_best_matches_list_tracks_count() {
        let pattern = create_pattern("rs");
        let matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        let mut list = BestMatchesList::new(2, pattern, matcher);

        list.insert("file1.rs");
        list.insert("file2.rs");
        list.insert("file3.rs");

        // num_matches tracks all matches, not just kept ones
        assert_eq!(list.num_matches, 3);
    }

    #[test]
    fn test_pattern_case_insensitive_smart() {
        let pattern = create_pattern("TEST");
        let mut utf32buf = Vec::new();
        let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
        // Smart case: uppercase pattern matches only uppercase (typically)
        let haystack: Utf32Str<'_> = Utf32Str::new("TEST", &mut utf32buf);
        assert!(pattern.score(haystack, &mut matcher).is_some());
    }

    #[test]
    fn test_file_match_clone() {
        let original = FileMatch {
            score: 80,
            path: "clone.rs".to_string(),
            indices: Some(vec![0, 1]),
        };
        let cloned = original.clone();
        assert_eq!(cloned.score, original.score);
        assert_eq!(cloned.path, original.path);
        assert_eq!(cloned.indices, original.indices);
    }

    #[test]
    fn test_search_config_clone() {
        let original = SearchConfig {
            limit: 25,
            compute_indices: true,
            respect_gitignore: false,
            exclude: vec!["*.tmp".to_string()],
            threads: 4,
            respect_codexignore: true,
        };
        let cloned = original.clone();
        assert_eq!(cloned.limit, original.limit);
        assert_eq!(cloned.compute_indices, original.compute_indices);
        assert_eq!(cloned.exclude, original.exclude);
        assert_eq!(cloned.respect_codexignore, original.respect_codexignore);
    }

    #[tokio::test]
    async fn test_search_async() {
        let config = SearchConfig {
            limit: 5,
            ..Default::default()
        };

        let result = search_async("Cargo", Path::new("."), &config, None).await;
        assert!(result.is_ok());

        let results = result.unwrap();
        // total_match_count is usize, just verify search completed
        let _ = results.total_match_count;
    }

    #[test]
    fn test_search_nonexistent_directory() {
        let config = SearchConfig::default();
        let result = search("test", Path::new("/nonexistent/path/12345"), &config, None);
        // Should handle gracefully (may error or return empty)
        // The behavior depends on the implementation
        let _ = result;
    }

    #[test]
    fn test_file_search_results_empty() {
        let results = FileSearchResults {
            matches: vec![],
            total_match_count: 0,
        };
        assert!(results.matches.is_empty());
        assert_eq!(results.total_match_count, 0);
    }

    #[test]
    fn test_file_match_debug() {
        let file_match = FileMatch {
            score: 90,
            path: "debug.rs".to_string(),
            indices: None,
        };
        let debug_str = format!("{:?}", file_match);
        assert!(debug_str.contains("FileMatch"));
        assert!(debug_str.contains("90"));
        assert!(debug_str.contains("debug.rs"));
    }

    #[test]
    fn test_file_search_results_debug() {
        let results = FileSearchResults {
            matches: vec![],
            total_match_count: 5,
        };
        let debug_str = format!("{:?}", results);
        assert!(debug_str.contains("FileSearchResults"));
    }

    #[test]
    fn test_search_config_debug() {
        let config = SearchConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("SearchConfig"));
    }
}
