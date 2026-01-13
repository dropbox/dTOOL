//! This module is responsible for parsing & validating a patch into a list of "hunks".
//! (It does not attempt to actually check that the patch can be applied to the filesystem.)
//!
//! The official Lark grammar for the apply-patch format is:
//!
//! start: begin_patch hunk+ end_patch
//! begin_patch: "*** Begin Patch" LF
//! end_patch: "*** End Patch" LF?
//!
//! hunk: add_hunk | delete_hunk | update_hunk
//! add_hunk: "*** Add File: " filename LF add_line+
//! delete_hunk: "*** Delete File: " filename LF
//! update_hunk: "*** Update File: " filename LF change_move? change?
//! filename: /(.+)/
//! add_line: "+" /(.+)/ LF -> line
//!
//! change_move: "*** Move to: " filename LF
//! change: (change_context | change_line)+ eof_line?
//! change_context: ("@@" | "@@ " /(.+)/) LF
//! change_line: ("+" | "-" | " ") /(.+)/ LF
//! eof_line: "*** End of File" LF
//!
//! The parser below is a little more lenient than the explicit spec and allows for
//! leading/trailing whitespace around patch markers.
use crate::ApplyPatchArgs;
use std::path::Path;
use std::path::PathBuf;

use thiserror::Error;

const BEGIN_PATCH_MARKER: &str = "*** Begin Patch";
const END_PATCH_MARKER: &str = "*** End Patch";
const ADD_FILE_MARKER: &str = "*** Add File: ";
const DELETE_FILE_MARKER: &str = "*** Delete File: ";
const UPDATE_FILE_MARKER: &str = "*** Update File: ";
const MOVE_TO_MARKER: &str = "*** Move to: ";
const EOF_MARKER: &str = "*** End of File";
const CHANGE_CONTEXT_MARKER: &str = "@@ ";
const EMPTY_CHANGE_CONTEXT_MARKER: &str = "@@";

/// Currently, the only OpenAI model that knowingly requires lenient parsing is
/// gpt-4.1. While we could try to require everyone to pass in a strictness
/// param when invoking apply_patch, it is a pain to thread it through all of
/// the call sites, so we resign ourselves allowing lenient parsing for all
/// models. See [`ParseMode::Lenient`] for details on the exceptions we make for
/// gpt-4.1.
const PARSE_IN_STRICT_MODE: bool = false;

#[derive(Debug, PartialEq, Error, Clone)]
pub enum ParseError {
    #[error("invalid patch: {0}")]
    InvalidPatchError(String),
    #[error("invalid hunk at line {line_number}, {message}")]
    InvalidHunkError { message: String, line_number: usize },
}
use ParseError::*;

#[derive(Debug, PartialEq, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum Hunk {
    AddFile {
        path: PathBuf,
        contents: String,
    },
    DeleteFile {
        path: PathBuf,
    },
    UpdateFile {
        path: PathBuf,
        move_path: Option<PathBuf>,

        /// Chunks should be in order, i.e. the `change_context` of one chunk
        /// should occur later in the file than the previous chunk.
        chunks: Vec<UpdateFileChunk>,
    },
}

impl Hunk {
    pub fn resolve_path(&self, cwd: &Path) -> PathBuf {
        match self {
            Hunk::AddFile { path, .. } => cwd.join(path),
            Hunk::DeleteFile { path } => cwd.join(path),
            Hunk::UpdateFile { path, .. } => cwd.join(path),
        }
    }
}

use Hunk::*;

#[derive(Debug, PartialEq, Clone)]
pub struct UpdateFileChunk {
    /// A single line of context used to narrow down the position of the chunk
    /// (this is usually a class, method, or function definition.)
    pub change_context: Option<String>,

    /// A contiguous block of lines that should be replaced with `new_lines`.
    /// `old_lines` must occur strictly after `change_context`.
    pub old_lines: Vec<String>,
    pub new_lines: Vec<String>,

    /// If set to true, `old_lines` must occur at the end of the source file.
    /// (Tolerance around trailing newlines should be encouraged.)
    pub is_end_of_file: bool,
}

pub fn parse_patch(patch: &str) -> Result<ApplyPatchArgs, ParseError> {
    let mode = if PARSE_IN_STRICT_MODE {
        ParseMode::Strict
    } else {
        ParseMode::Lenient
    };
    parse_patch_text(patch, mode)
}

enum ParseMode {
    /// Parse the patch text argument as is.
    Strict,

    /// GPT-4.1 is known to formulate the `command` array for the `local_shell`
    /// tool call for `apply_patch` call using something like the following:
    ///
    /// ```json
    /// [
    ///   "apply_patch",
    ///   "<<'EOF'\n*** Begin Patch\n*** Update File: README.md\n@@...\n*** End Patch\nEOF\n",
    /// ]
    /// ```
    ///
    /// This is a problem because `local_shell` is a bit of a misnomer: the
    /// `command` is not invoked by passing the arguments to a shell like Bash,
    /// but are invoked using something akin to `execvpe(3)`.
    ///
    /// This is significant in this case because where a shell would interpret
    /// `<<'EOF'...` as a heredoc and pass the contents via stdin (which is
    /// fine, as `apply_patch` is specified to read from stdin if no argument is
    /// passed), `execvpe(3)` interprets the heredoc as a literal string. To get
    /// the `local_shell` tool to run a command the way shell would, the
    /// `command` array must be something like:
    ///
    /// ```json
    /// [
    ///   "bash",
    ///   "-lc",
    ///   "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: README.md\n@@...\n*** End Patch\nEOF\n",
    /// ]
    /// ```
    ///
    /// In lenient mode, we check if the argument to `apply_patch` starts with
    /// `<<'EOF'` and ends with `EOF\n`. If so, we strip off these markers,
    /// trim() the result, and treat what is left as the patch text.
    Lenient,
}

fn parse_patch_text(patch: &str, mode: ParseMode) -> Result<ApplyPatchArgs, ParseError> {
    let lines: Vec<&str> = patch.trim().lines().collect();
    let lines: &[&str] = match check_patch_boundaries_strict(&lines) {
        Ok(()) => &lines,
        Err(e) => match mode {
            ParseMode::Strict => {
                return Err(e);
            }
            ParseMode::Lenient => check_patch_boundaries_lenient(&lines, e)?,
        },
    };

    let mut hunks: Vec<Hunk> = Vec::new();
    // The above checks ensure that lines.len() >= 2.
    let last_line_index = lines.len().saturating_sub(1);
    let mut remaining_lines = &lines[1..last_line_index];
    let mut line_number = 2;
    while !remaining_lines.is_empty() {
        let (hunk, hunk_lines) = parse_one_hunk(remaining_lines, line_number)?;
        hunks.push(hunk);
        line_number += hunk_lines;
        remaining_lines = &remaining_lines[hunk_lines..]
    }
    let patch = lines.join("\n");
    Ok(ApplyPatchArgs {
        hunks,
        patch,
        workdir: None,
    })
}

/// Checks the start and end lines of the patch text for `apply_patch`,
/// returning an error if they do not match the expected markers.
fn check_patch_boundaries_strict(lines: &[&str]) -> Result<(), ParseError> {
    let (first_line, last_line) = match lines {
        [] => (None, None),
        [first] => (Some(first), Some(first)),
        [first, .., last] => (Some(first), Some(last)),
    };
    check_start_and_end_lines_strict(first_line, last_line)
}

/// If we are in lenient mode, we check if the first line starts with `<<EOF`
/// (possibly quoted) and the last line ends with `EOF`. There must be at least
/// 4 lines total because the heredoc markers take up 2 lines and the patch text
/// must have at least 2 lines.
///
/// If successful, returns the lines of the patch text that contain the patch
/// contents, excluding the heredoc markers.
fn check_patch_boundaries_lenient<'a>(
    original_lines: &'a [&'a str],
    original_parse_error: ParseError,
) -> Result<&'a [&'a str], ParseError> {
    match original_lines {
        [first, .., last] => {
            if (first == &"<<EOF" || first == &"<<'EOF'" || first == &"<<\"EOF\"")
                && last.ends_with("EOF")
                && original_lines.len() >= 4
            {
                let inner_lines = &original_lines[1..original_lines.len() - 1];
                match check_patch_boundaries_strict(inner_lines) {
                    Ok(()) => Ok(inner_lines),
                    Err(e) => Err(e),
                }
            } else {
                Err(original_parse_error)
            }
        }
        _ => Err(original_parse_error),
    }
}

fn check_start_and_end_lines_strict(
    first_line: Option<&&str>,
    last_line: Option<&&str>,
) -> Result<(), ParseError> {
    match (first_line, last_line) {
        (Some(&first), Some(&last)) if first == BEGIN_PATCH_MARKER && last == END_PATCH_MARKER => {
            Ok(())
        }
        (Some(&first), _) if first != BEGIN_PATCH_MARKER => Err(InvalidPatchError(String::from(
            "The first line of the patch must be '*** Begin Patch'",
        ))),
        _ => Err(InvalidPatchError(String::from(
            "The last line of the patch must be '*** End Patch'",
        ))),
    }
}

/// Attempts to parse a single hunk from the start of lines.
/// Returns the parsed hunk and the number of lines parsed (or a ParseError).
fn parse_one_hunk(lines: &[&str], line_number: usize) -> Result<(Hunk, usize), ParseError> {
    // Be tolerant of case mismatches and extra padding around marker strings.
    let first_line = lines[0].trim();
    if let Some(path) = first_line.strip_prefix(ADD_FILE_MARKER) {
        // Add File
        let mut contents = String::new();
        let mut parsed_lines = 1;
        for add_line in &lines[1..] {
            if let Some(line_to_add) = add_line.strip_prefix('+') {
                contents.push_str(line_to_add);
                contents.push('\n');
                parsed_lines += 1;
            } else {
                break;
            }
        }
        return Ok((
            AddFile {
                path: PathBuf::from(path),
                contents,
            },
            parsed_lines,
        ));
    } else if let Some(path) = first_line.strip_prefix(DELETE_FILE_MARKER) {
        // Delete File
        return Ok((
            DeleteFile {
                path: PathBuf::from(path),
            },
            1,
        ));
    } else if let Some(path) = first_line.strip_prefix(UPDATE_FILE_MARKER) {
        // Update File
        let mut remaining_lines = &lines[1..];
        let mut parsed_lines = 1;

        // Optional: move file line
        let move_path = remaining_lines
            .first()
            .and_then(|x| x.strip_prefix(MOVE_TO_MARKER));

        if move_path.is_some() {
            remaining_lines = &remaining_lines[1..];
            parsed_lines += 1;
        }

        let mut chunks = Vec::new();
        // NOTE: we need to know to stop once we reach the next special marker header.
        while !remaining_lines.is_empty() {
            // Skip over any completely blank lines that may separate chunks.
            if remaining_lines[0].trim().is_empty() {
                parsed_lines += 1;
                remaining_lines = &remaining_lines[1..];
                continue;
            }

            if remaining_lines[0].starts_with("***") {
                break;
            }

            let (chunk, chunk_lines) = parse_update_file_chunk(
                remaining_lines,
                line_number + parsed_lines,
                chunks.is_empty(),
            )?;
            chunks.push(chunk);
            parsed_lines += chunk_lines;
            remaining_lines = &remaining_lines[chunk_lines..]
        }

        if chunks.is_empty() {
            return Err(InvalidHunkError {
                message: format!("Update file hunk for path '{path}' is empty"),
                line_number,
            });
        }

        return Ok((
            UpdateFile {
                path: PathBuf::from(path),
                move_path: move_path.map(PathBuf::from),
                chunks,
            },
            parsed_lines,
        ));
    }

    Err(InvalidHunkError {
        message: format!(
            "'{first_line}' is not a valid hunk header. Valid hunk headers: '*** Add File: {{path}}', '*** Delete File: {{path}}', '*** Update File: {{path}}'"
        ),
        line_number,
    })
}

fn parse_update_file_chunk(
    lines: &[&str],
    line_number: usize,
    allow_missing_context: bool,
) -> Result<(UpdateFileChunk, usize), ParseError> {
    if lines.is_empty() {
        return Err(InvalidHunkError {
            message: "Update hunk does not contain any lines".to_string(),
            line_number,
        });
    }
    // If we see an explicit context marker @@ or @@ <context>, consume it; otherwise, optionally
    // allow treating the chunk as starting directly with diff lines.
    let (change_context, start_index) = if lines[0] == EMPTY_CHANGE_CONTEXT_MARKER {
        (None, 1)
    } else if let Some(context) = lines[0].strip_prefix(CHANGE_CONTEXT_MARKER) {
        (Some(context.to_string()), 1)
    } else {
        if !allow_missing_context {
            return Err(InvalidHunkError {
                message: format!(
                    "Expected update hunk to start with a @@ context marker, got: '{}'",
                    lines[0]
                ),
                line_number,
            });
        }
        (None, 0)
    };
    if start_index >= lines.len() {
        return Err(InvalidHunkError {
            message: "Update hunk does not contain any lines".to_string(),
            line_number: line_number + 1,
        });
    }
    let mut chunk = UpdateFileChunk {
        change_context,
        old_lines: Vec::new(),
        new_lines: Vec::new(),
        is_end_of_file: false,
    };
    let mut parsed_lines = 0;
    for line in &lines[start_index..] {
        match *line {
            EOF_MARKER => {
                if parsed_lines == 0 {
                    return Err(InvalidHunkError {
                        message: "Update hunk does not contain any lines".to_string(),
                        line_number: line_number + 1,
                    });
                }
                chunk.is_end_of_file = true;
                parsed_lines += 1;
                break;
            }
            line_contents => {
                match line_contents.chars().next() {
                    None => {
                        // Interpret this as an empty line.
                        chunk.old_lines.push(String::new());
                        chunk.new_lines.push(String::new());
                    }
                    Some(' ') => {
                        chunk.old_lines.push(line_contents[1..].to_string());
                        chunk.new_lines.push(line_contents[1..].to_string());
                    }
                    Some('+') => {
                        chunk.new_lines.push(line_contents[1..].to_string());
                    }
                    Some('-') => {
                        chunk.old_lines.push(line_contents[1..].to_string());
                    }
                    _ => {
                        if parsed_lines == 0 {
                            return Err(InvalidHunkError {
                                message: format!(
                                    "Unexpected line found in update hunk: '{line_contents}'. Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)"
                                ),
                                line_number: line_number + 1,
                            });
                        }
                        // Assume this is the start of the next hunk.
                        break;
                    }
                }
                parsed_lines += 1;
            }
        }
    }

    Ok((chunk, parsed_lines + start_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_patch() {
        assert_eq!(
            parse_patch_text("bad", ParseMode::Strict),
            Err(InvalidPatchError(
                "The first line of the patch must be '*** Begin Patch'".to_string()
            ))
        );
        assert_eq!(
            parse_patch_text("*** Begin Patch\nbad", ParseMode::Strict),
            Err(InvalidPatchError(
                "The last line of the patch must be '*** End Patch'".to_string()
            ))
        );
        assert_eq!(
            parse_patch_text(
                "*** Begin Patch\n\
                 *** Update File: test.py\n\
                 *** End Patch",
                ParseMode::Strict
            ),
            Err(InvalidHunkError {
                message: "Update file hunk for path 'test.py' is empty".to_string(),
                line_number: 2,
            })
        );
        assert_eq!(
            parse_patch_text(
                "*** Begin Patch\n\
                 *** End Patch",
                ParseMode::Strict
            )
            .unwrap()
            .hunks,
            Vec::new()
        );
        assert_eq!(
            parse_patch_text(
                "*** Begin Patch\n\
                 *** Add File: path/add.py\n\
                 +abc\n\
                 +def\n\
                 *** Delete File: path/delete.py\n\
                 *** Update File: path/update.py\n\
                 *** Move to: path/update2.py\n\
                 @@ def f():\n\
                 -    pass\n\
                 +    return 123\n\
                 *** End Patch",
                ParseMode::Strict
            )
            .unwrap()
            .hunks,
            vec![
                AddFile {
                    path: PathBuf::from("path/add.py"),
                    contents: "abc\ndef\n".to_string()
                },
                DeleteFile {
                    path: PathBuf::from("path/delete.py")
                },
                UpdateFile {
                    path: PathBuf::from("path/update.py"),
                    move_path: Some(PathBuf::from("path/update2.py")),
                    chunks: vec![UpdateFileChunk {
                        change_context: Some("def f():".to_string()),
                        old_lines: vec!["    pass".to_string()],
                        new_lines: vec!["    return 123".to_string()],
                        is_end_of_file: false
                    }]
                }
            ]
        );
        // Update hunk followed by another hunk (Add File).
        assert_eq!(
            parse_patch_text(
                "*** Begin Patch\n\
                 *** Update File: file.py\n\
                 @@\n\
                 +line\n\
                 *** Add File: other.py\n\
                 +content\n\
                 *** End Patch",
                ParseMode::Strict
            )
            .unwrap()
            .hunks,
            vec![
                UpdateFile {
                    path: PathBuf::from("file.py"),
                    move_path: None,
                    chunks: vec![UpdateFileChunk {
                        change_context: None,
                        old_lines: vec![],
                        new_lines: vec!["line".to_string()],
                        is_end_of_file: false
                    }],
                },
                AddFile {
                    path: PathBuf::from("other.py"),
                    contents: "content\n".to_string()
                }
            ]
        );

        // Update hunk without an explicit @@ header for the first chunk should parse.
        // Use a raw string to preserve the leading space diff marker on the context line.
        assert_eq!(
            parse_patch_text(
                r#"*** Begin Patch
*** Update File: file2.py
 import foo
+bar
*** End Patch"#,
                ParseMode::Strict
            )
            .unwrap()
            .hunks,
            vec![UpdateFile {
                path: PathBuf::from("file2.py"),
                move_path: None,
                chunks: vec![UpdateFileChunk {
                    change_context: None,
                    old_lines: vec!["import foo".to_string()],
                    new_lines: vec!["import foo".to_string(), "bar".to_string()],
                    is_end_of_file: false,
                }],
            }]
        );
    }

    #[test]
    fn test_parse_patch_lenient() {
        let patch_text = r#"*** Begin Patch
*** Update File: file2.py
 import foo
+bar
*** End Patch"#;
        let expected_patch = vec![UpdateFile {
            path: PathBuf::from("file2.py"),
            move_path: None,
            chunks: vec![UpdateFileChunk {
                change_context: None,
                old_lines: vec!["import foo".to_string()],
                new_lines: vec!["import foo".to_string(), "bar".to_string()],
                is_end_of_file: false,
            }],
        }];
        let expected_error =
            InvalidPatchError("The first line of the patch must be '*** Begin Patch'".to_string());

        let patch_text_in_heredoc = format!("<<EOF\n{patch_text}\nEOF\n");
        assert_eq!(
            parse_patch_text(&patch_text_in_heredoc, ParseMode::Strict),
            Err(expected_error.clone())
        );
        assert_eq!(
            parse_patch_text(&patch_text_in_heredoc, ParseMode::Lenient),
            Ok(ApplyPatchArgs {
                hunks: expected_patch.clone(),
                patch: patch_text.to_string(),
                workdir: None,
            })
        );

        let patch_text_in_single_quoted_heredoc = format!("<<'EOF'\n{patch_text}\nEOF\n");
        assert_eq!(
            parse_patch_text(&patch_text_in_single_quoted_heredoc, ParseMode::Strict),
            Err(expected_error.clone())
        );
        assert_eq!(
            parse_patch_text(&patch_text_in_single_quoted_heredoc, ParseMode::Lenient),
            Ok(ApplyPatchArgs {
                hunks: expected_patch.clone(),
                patch: patch_text.to_string(),
                workdir: None,
            })
        );

        let patch_text_in_double_quoted_heredoc = format!("<<\"EOF\"\n{patch_text}\nEOF\n");
        assert_eq!(
            parse_patch_text(&patch_text_in_double_quoted_heredoc, ParseMode::Strict),
            Err(expected_error.clone())
        );
        assert_eq!(
            parse_patch_text(&patch_text_in_double_quoted_heredoc, ParseMode::Lenient),
            Ok(ApplyPatchArgs {
                hunks: expected_patch,
                patch: patch_text.to_string(),
                workdir: None,
            })
        );

        let patch_text_in_mismatched_quotes_heredoc = format!("<<\"EOF'\n{patch_text}\nEOF\n");
        assert_eq!(
            parse_patch_text(&patch_text_in_mismatched_quotes_heredoc, ParseMode::Strict),
            Err(expected_error.clone())
        );
        assert_eq!(
            parse_patch_text(&patch_text_in_mismatched_quotes_heredoc, ParseMode::Lenient),
            Err(expected_error.clone())
        );

        let patch_text_with_missing_closing_heredoc =
            "<<EOF\n*** Begin Patch\n*** Update File: file2.py\nEOF\n".to_string();
        assert_eq!(
            parse_patch_text(&patch_text_with_missing_closing_heredoc, ParseMode::Strict),
            Err(expected_error)
        );
        assert_eq!(
            parse_patch_text(&patch_text_with_missing_closing_heredoc, ParseMode::Lenient),
            Err(InvalidPatchError(
                "The last line of the patch must be '*** End Patch'".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_one_hunk() {
        assert_eq!(
            parse_one_hunk(&["bad"], 234),
            Err(InvalidHunkError {
                message: "'bad' is not a valid hunk header. \
            Valid hunk headers: '*** Add File: {path}', '*** Delete File: {path}', '*** Update File: {path}'".to_string(),
                line_number: 234
            })
        );
        // Other edge cases are already covered by tests above/below.
    }

    #[test]
    fn test_update_file_chunk() {
        assert_eq!(
            parse_update_file_chunk(&["bad"], 123, false),
            Err(InvalidHunkError {
                message: "Expected update hunk to start with a @@ context marker, got: 'bad'"
                    .to_string(),
                line_number: 123
            })
        );
        assert_eq!(
            parse_update_file_chunk(&["@@"], 123, false),
            Err(InvalidHunkError {
                message: "Update hunk does not contain any lines".to_string(),
                line_number: 124
            })
        );
        assert_eq!(
            parse_update_file_chunk(&["@@", "bad"], 123, false),
            Err(InvalidHunkError {
                message:  "Unexpected line found in update hunk: 'bad'. \
                           Every line should start with ' ' (context line), '+' (added line), or '-' (removed line)".to_string(),
                line_number: 124
            })
        );
        assert_eq!(
            parse_update_file_chunk(&["@@", "*** End of File"], 123, false),
            Err(InvalidHunkError {
                message: "Update hunk does not contain any lines".to_string(),
                line_number: 124
            })
        );
        assert_eq!(
            parse_update_file_chunk(
                &[
                    "@@ change_context",
                    "",
                    " context",
                    "-remove",
                    "+add",
                    " context2",
                    "*** End Patch",
                ],
                123,
                false
            ),
            Ok((
                (UpdateFileChunk {
                    change_context: Some("change_context".to_string()),
                    old_lines: vec![
                        "".to_string(),
                        "context".to_string(),
                        "remove".to_string(),
                        "context2".to_string()
                    ],
                    new_lines: vec![
                        "".to_string(),
                        "context".to_string(),
                        "add".to_string(),
                        "context2".to_string()
                    ],
                    is_end_of_file: false
                }),
                6
            ))
        );
        assert_eq!(
            parse_update_file_chunk(&["@@", "+line", "*** End of File"], 123, false),
            Ok((
                (UpdateFileChunk {
                    change_context: None,
                    old_lines: vec![],
                    new_lines: vec!["line".to_string()],
                    is_end_of_file: true
                }),
                3
            ))
        );
    }

    // === Additional tests for improved coverage ===

    // --- ParseError tests ---

    #[test]
    fn test_parse_error_invalid_patch_display() {
        let err = InvalidPatchError("test error message".to_string());
        assert_eq!(err.to_string(), "invalid patch: test error message");
    }

    #[test]
    fn test_parse_error_invalid_hunk_display() {
        let err = InvalidHunkError {
            message: "bad hunk".to_string(),
            line_number: 42,
        };
        assert_eq!(err.to_string(), "invalid hunk at line 42, bad hunk");
    }

    #[test]
    fn test_parse_error_clone() {
        let err = InvalidPatchError("original".to_string());
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_parse_error_debug() {
        let err = InvalidHunkError {
            message: "msg".to_string(),
            line_number: 10,
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("InvalidHunkError"));
        assert!(debug.contains("msg"));
        assert!(debug.contains("10"));
    }

    // --- Hunk tests ---

    #[test]
    fn test_hunk_resolve_path_add_file() {
        let hunk = AddFile {
            path: PathBuf::from("subdir/file.txt"),
            contents: "content".to_string(),
        };
        let resolved = hunk.resolve_path(Path::new("/project"));
        assert_eq!(resolved, PathBuf::from("/project/subdir/file.txt"));
    }

    #[test]
    fn test_hunk_resolve_path_delete_file() {
        let hunk = DeleteFile {
            path: PathBuf::from("old_file.py"),
        };
        let resolved = hunk.resolve_path(Path::new("/home/user/code"));
        assert_eq!(resolved, PathBuf::from("/home/user/code/old_file.py"));
    }

    #[test]
    fn test_hunk_resolve_path_update_file() {
        let hunk = UpdateFile {
            path: PathBuf::from("src/main.rs"),
            move_path: None,
            chunks: vec![],
        };
        let resolved = hunk.resolve_path(Path::new("/rust/project"));
        assert_eq!(resolved, PathBuf::from("/rust/project/src/main.rs"));
    }

    #[test]
    fn test_hunk_resolve_path_with_empty_cwd() {
        let hunk = AddFile {
            path: PathBuf::from("file.txt"),
            contents: String::new(),
        };
        let resolved = hunk.resolve_path(Path::new(""));
        assert_eq!(resolved, PathBuf::from("file.txt"));
    }

    #[test]
    fn test_hunk_clone() {
        let hunk = AddFile {
            path: PathBuf::from("test.txt"),
            contents: "hello".to_string(),
        };
        let cloned = hunk.clone();
        assert_eq!(hunk, cloned);
    }

    #[test]
    fn test_hunk_debug() {
        let hunk = DeleteFile {
            path: PathBuf::from("file.txt"),
        };
        let debug = format!("{:?}", hunk);
        assert!(debug.contains("DeleteFile"));
        assert!(debug.contains("file.txt"));
    }

    // --- UpdateFileChunk tests ---

    #[test]
    fn test_update_file_chunk_clone() {
        let chunk = UpdateFileChunk {
            change_context: Some("fn foo()".to_string()),
            old_lines: vec!["old".to_string()],
            new_lines: vec!["new".to_string()],
            is_end_of_file: false,
        };
        let cloned = chunk.clone();
        assert_eq!(chunk, cloned);
    }

    #[test]
    fn test_update_file_chunk_debug() {
        let chunk = UpdateFileChunk {
            change_context: None,
            old_lines: vec![],
            new_lines: vec!["new line".to_string()],
            is_end_of_file: true,
        };
        let debug = format!("{:?}", chunk);
        assert!(debug.contains("UpdateFileChunk"));
        assert!(debug.contains("new line"));
        assert!(debug.contains("true"));
    }

    // --- parse_patch (public API) tests ---

    #[test]
    fn test_parse_patch_public_api() {
        // Test the public parse_patch function (uses lenient mode by default)
        let patch = "*** Begin Patch\n*** Delete File: old.txt\n*** End Patch";
        let result = parse_patch(patch);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.hunks.len(), 1);
        assert!(matches!(&args.hunks[0], DeleteFile { path } if path == Path::new("old.txt")));
    }

    #[test]
    fn test_parse_patch_returns_patch_text() {
        let patch = "*** Begin Patch\n*** Delete File: x.txt\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert!(result.patch.contains("*** Begin Patch"));
        assert!(result.patch.contains("*** End Patch"));
    }

    #[test]
    fn test_parse_patch_workdir_is_none() {
        let patch = "*** Begin Patch\n*** Delete File: x.txt\n*** End Patch";
        let result = parse_patch(patch).unwrap();
        assert!(result.workdir.is_none());
    }

    // --- Strict mode edge cases ---

    #[test]
    fn test_strict_mode_empty_input() {
        let result = parse_patch_text("", ParseMode::Strict);
        assert!(result.is_err());
    }

    #[test]
    fn test_strict_mode_single_line() {
        let result = parse_patch_text("*** Begin Patch", ParseMode::Strict);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, InvalidPatchError(_)));
    }

    #[test]
    fn test_strict_mode_wrong_first_line() {
        let result = parse_patch_text("Not a patch\n*** End Patch", ParseMode::Strict);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            InvalidPatchError(msg) => {
                assert!(msg.contains("first line"));
            }
            _ => panic!("Expected InvalidPatchError"),
        }
    }

    #[test]
    fn test_strict_mode_wrong_last_line() {
        let result = parse_patch_text("*** Begin Patch\nNot an end marker", ParseMode::Strict);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            InvalidPatchError(msg) => {
                assert!(msg.contains("last line"));
            }
            _ => panic!("Expected InvalidPatchError"),
        }
    }

    // --- Lenient mode heredoc variations ---

    #[test]
    fn test_lenient_mode_heredoc_with_trailing_content() {
        // The EOF marker can have trailing content (e.g., "EOF\n" vs just "EOF")
        let patch_text = "*** Begin Patch\n*** Delete File: x.txt\n*** End Patch";
        let with_heredoc = format!("<<EOF\n{}\nEOF", patch_text);
        let result = parse_patch_text(&with_heredoc, ParseMode::Lenient);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lenient_mode_too_few_lines_in_heredoc() {
        // Heredoc with only 3 lines total (needs 4: heredoc start, begin patch, end patch, heredoc end)
        let patch = "<<EOF\n*** Begin Patch\nEOF";
        let result = parse_patch_text(patch, ParseMode::Lenient);
        assert!(result.is_err());
    }

    #[test]
    fn test_lenient_mode_wrong_heredoc_start() {
        let patch = "<<WRONG\n*** Begin Patch\n*** End Patch\nWRONG";
        let result = parse_patch_text(patch, ParseMode::Lenient);
        assert!(result.is_err());
    }

    #[test]
    fn test_lenient_mode_heredoc_single_quoted_multiline() {
        let patch =
            "<<'EOF'\n*** Begin Patch\n*** Add File: test.txt\n+line1\n+line2\n*** End Patch\nEOF";
        let result = parse_patch_text(patch, ParseMode::Lenient);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.hunks.len(), 1);
    }

    // --- parse_one_hunk edge cases ---

    #[test]
    fn test_parse_one_hunk_add_file_empty_content() {
        let lines = ["*** Add File: empty.txt"];
        let result = parse_one_hunk(&lines, 1);
        assert!(result.is_ok());
        let (hunk, count) = result.unwrap();
        assert_eq!(count, 1);
        match hunk {
            AddFile { path, contents } => {
                assert_eq!(path, PathBuf::from("empty.txt"));
                assert!(contents.is_empty());
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_parse_one_hunk_add_file_multiple_lines() {
        let lines = [
            "*** Add File: multi.txt",
            "+line1",
            "+line2",
            "+line3",
            "*** Delete File: next.txt", // Next hunk marker
        ];
        let result = parse_one_hunk(&lines, 1);
        assert!(result.is_ok());
        let (hunk, count) = result.unwrap();
        assert_eq!(count, 4); // Header + 3 add lines
        match hunk {
            AddFile { path, contents } => {
                assert_eq!(path, PathBuf::from("multi.txt"));
                assert_eq!(contents, "line1\nline2\nline3\n");
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_parse_one_hunk_add_file_with_special_chars() {
        let lines = [
            "*** Add File: path/to/file.rs",
            "+fn main() {",
            "+    println!(\"Hello, World!\");",
            "+}",
        ];
        let result = parse_one_hunk(&lines, 1);
        assert!(result.is_ok());
        let (hunk, _) = result.unwrap();
        match hunk {
            AddFile { contents, .. } => {
                assert!(contents.contains("fn main()"));
                assert!(contents.contains("println!"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_parse_one_hunk_delete_file() {
        let lines = ["*** Delete File: obsolete.py"];
        let result = parse_one_hunk(&lines, 5);
        assert!(result.is_ok());
        let (hunk, count) = result.unwrap();
        assert_eq!(count, 1);
        match hunk {
            DeleteFile { path } => {
                assert_eq!(path, PathBuf::from("obsolete.py"));
            }
            _ => panic!("Expected DeleteFile"),
        }
    }

    #[test]
    fn test_parse_one_hunk_update_file_with_move() {
        let lines = [
            "*** Update File: old_name.rs",
            "*** Move to: new_name.rs",
            "@@ fn foo()",
            " context",
            "-old",
            "+new",
        ];
        let result = parse_one_hunk(&lines, 1);
        assert!(result.is_ok());
        let (hunk, _) = result.unwrap();
        match hunk {
            UpdateFile {
                path,
                move_path,
                chunks,
            } => {
                assert_eq!(path, PathBuf::from("old_name.rs"));
                assert_eq!(move_path, Some(PathBuf::from("new_name.rs")));
                assert_eq!(chunks.len(), 1);
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_one_hunk_update_file_blank_lines_between_chunks() {
        let lines = [
            "*** Update File: file.py",
            "@@ first chunk",
            "+add1",
            "",
            "@@ second chunk",
            "+add2",
        ];
        let result = parse_one_hunk(&lines, 1);
        assert!(result.is_ok());
        let (hunk, _) = result.unwrap();
        match hunk {
            UpdateFile { chunks, .. } => {
                // Blank line between chunks should be skipped
                assert_eq!(chunks.len(), 2);
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_one_hunk_invalid_header() {
        let lines = ["Not a valid marker"];
        let result = parse_one_hunk(&lines, 100);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            InvalidHunkError {
                message,
                line_number,
            } => {
                assert_eq!(line_number, 100);
                assert!(message.contains("not a valid hunk header"));
            }
            _ => panic!("Expected InvalidHunkError"),
        }
    }

    #[test]
    fn test_parse_one_hunk_whitespace_trimming() {
        // The parser trims the first line
        let lines = ["  *** Add File: trimmed.txt  ", "+content"];
        let result = parse_one_hunk(&lines, 1);
        assert!(result.is_ok());
        let (hunk, _) = result.unwrap();
        match hunk {
            AddFile { path, .. } => {
                assert_eq!(path, PathBuf::from("trimmed.txt"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    // --- parse_update_file_chunk additional tests ---

    #[test]
    fn test_parse_update_file_chunk_empty_lines_array() {
        let lines: [&str; 0] = [];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            InvalidHunkError { message, .. } => {
                assert!(message.contains("does not contain any lines"));
            }
            _ => panic!("Expected InvalidHunkError"),
        }
    }

    #[test]
    fn test_parse_update_file_chunk_allow_missing_context() {
        // When allow_missing_context is true, can start directly with diff lines
        let lines = [" context line", "+added", "-removed"];
        let result = parse_update_file_chunk(&lines, 1, true);
        assert!(result.is_ok());
        let (chunk, count) = result.unwrap();
        assert_eq!(count, 3);
        assert!(chunk.change_context.is_none());
        assert_eq!(chunk.old_lines.len(), 2); // context + removed
        assert_eq!(chunk.new_lines.len(), 2); // context + added
    }

    #[test]
    fn test_parse_update_file_chunk_context_only() {
        let lines = ["@@ fn main()", " unchanged line"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, _) = result.unwrap();
        assert_eq!(chunk.change_context, Some("fn main()".to_string()));
        assert_eq!(chunk.old_lines, vec!["unchanged line".to_string()]);
        assert_eq!(chunk.new_lines, vec!["unchanged line".to_string()]);
    }

    #[test]
    fn test_parse_update_file_chunk_add_only() {
        let lines = ["@@", "+new line 1", "+new line 2"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, _) = result.unwrap();
        assert!(chunk.old_lines.is_empty());
        assert_eq!(chunk.new_lines.len(), 2);
    }

    #[test]
    fn test_parse_update_file_chunk_remove_only() {
        let lines = ["@@", "-old line 1", "-old line 2"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, _) = result.unwrap();
        assert_eq!(chunk.old_lines.len(), 2);
        assert!(chunk.new_lines.is_empty());
    }

    #[test]
    fn test_parse_update_file_chunk_stops_at_next_hunk() {
        let lines = ["@@", "+add", "*** Add File: next.txt"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, count) = result.unwrap();
        assert_eq!(count, 2); // @@ and +add, stops before *** marker
        assert!(!chunk.is_end_of_file);
    }

    #[test]
    fn test_parse_update_file_chunk_eof_marker() {
        let lines = ["@@", "+last line", "*** End of File"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, count) = result.unwrap();
        assert_eq!(count, 3);
        assert!(chunk.is_end_of_file);
    }

    #[test]
    fn test_parse_update_file_chunk_mixed_changes() {
        let lines = [
            "@@ class MyClass:",
            " def __init__(self):",
            "-        self.old = True",
            "+        self.new = False",
            " def method(self):",
        ];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, _) = result.unwrap();
        assert_eq!(chunk.change_context, Some("class MyClass:".to_string()));
        assert_eq!(chunk.old_lines.len(), 3);
        assert_eq!(chunk.new_lines.len(), 3);
    }

    // --- Full patch parsing complex scenarios ---

    #[test]
    fn test_parse_patch_multiple_update_chunks() {
        let patch = r#"*** Begin Patch
*** Update File: file.py
@@ first function
-old1
+new1
@@ second function
-old2
+new2
*** End Patch"#;
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.hunks.len(), 1);
        match &args.hunks[0] {
            UpdateFile { chunks, .. } => {
                assert_eq!(chunks.len(), 2);
                assert_eq!(chunks[0].change_context, Some("first function".to_string()));
                assert_eq!(
                    chunks[1].change_context,
                    Some("second function".to_string())
                );
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_patch_all_hunk_types() {
        let patch = r#"*** Begin Patch
*** Add File: new.txt
+content
*** Delete File: old.txt
*** Update File: modify.txt
@@
-remove
+add
*** End Patch"#;
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        assert_eq!(args.hunks.len(), 3);
        assert!(matches!(&args.hunks[0], AddFile { .. }));
        assert!(matches!(&args.hunks[1], DeleteFile { .. }));
        assert!(matches!(&args.hunks[2], UpdateFile { .. }));
    }

    #[test]
    fn test_parse_patch_unicode_content() {
        let patch =
            "*** Begin Patch\n*** Add File: unicode.txt\n+你好世界\n+🦀 Rust\n*** End Patch";
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        match &args.hunks[0] {
            AddFile { contents, .. } => {
                assert!(contents.contains("你好世界"));
                assert!(contents.contains("🦀"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_parse_patch_deep_path() {
        let patch = "*** Begin Patch\n*** Add File: a/b/c/d/e/f/deep.txt\n+deep\n*** End Patch";
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        match &args.hunks[0] {
            AddFile { path, .. } => {
                assert_eq!(path, &PathBuf::from("a/b/c/d/e/f/deep.txt"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    #[test]
    fn test_parse_patch_file_with_spaces_in_name() {
        let patch =
            "*** Begin Patch\n*** Add File: path with spaces/my file.txt\n+content\n*** End Patch";
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        match &args.hunks[0] {
            AddFile { path, .. } => {
                assert_eq!(path, &PathBuf::from("path with spaces/my file.txt"));
            }
            _ => panic!("Expected AddFile"),
        }
    }

    // --- check_patch_boundaries tests ---

    #[test]
    fn test_check_patch_boundaries_empty_array() {
        let lines: Vec<&str> = vec![];
        let result = check_patch_boundaries_strict(&lines);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_patch_boundaries_single_element() {
        let lines = vec!["*** Begin Patch"];
        let result = check_patch_boundaries_strict(&lines);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_patch_boundaries_valid() {
        let lines = vec!["*** Begin Patch", "*** End Patch"];
        let result = check_patch_boundaries_strict(&lines);
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_patch_boundaries_with_content() {
        let lines = vec![
            "*** Begin Patch",
            "*** Add File: test.txt",
            "+content",
            "*** End Patch",
        ];
        let result = check_patch_boundaries_strict(&lines);
        assert!(result.is_ok());
    }

    // --- check_start_and_end_lines_strict tests ---

    #[test]
    fn test_check_start_and_end_lines_both_none() {
        let result = check_start_and_end_lines_strict(None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_start_and_end_lines_first_none() {
        let last = "*** End Patch";
        let result = check_start_and_end_lines_strict(None, Some(&last));
        assert!(result.is_err());
    }

    #[test]
    fn test_check_start_and_end_lines_valid() {
        let first = "*** Begin Patch";
        let last = "*** End Patch";
        let result = check_start_and_end_lines_strict(Some(&first), Some(&last));
        assert!(result.is_ok());
    }

    // --- Edge cases for empty/whitespace handling ---

    #[test]
    fn test_parse_patch_whitespace_only() {
        let result = parse_patch_text("   \n\t\n   ", ParseMode::Strict);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_patch_trimmed_markers() {
        // Note: trim() is called on the whole patch string, not individual lines for markers.
        // The first and last lines must exactly match the markers after the overall trim().
        // Leading/trailing whitespace on the first line IS trimmed by the outer trim().
        let patch = "  *** Begin Patch\n*** Delete File: x.txt\n*** End Patch  ";
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
    }

    #[test]
    fn test_update_chunk_empty_context_marker() {
        // Just "@@" without space
        let lines = ["@@", "+line"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, _) = result.unwrap();
        assert!(chunk.change_context.is_none());
    }

    #[test]
    fn test_update_chunk_context_with_space() {
        // "@@ " with trailing content
        let lines = ["@@ some context here", "+line"];
        let result = parse_update_file_chunk(&lines, 1, false);
        assert!(result.is_ok());
        let (chunk, _) = result.unwrap();
        assert_eq!(chunk.change_context, Some("some context here".to_string()));
    }

    // --- Complex real-world-like patches ---

    #[test]
    fn test_parse_patch_rust_code() {
        let patch = r#"*** Begin Patch
*** Update File: src/main.rs
@@ fn main()
 fn main() {
-    println!("Hello");
+    println!("Hello, World!");
 }
*** End Patch"#;
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        match &args.hunks[0] {
            UpdateFile { chunks, .. } => {
                assert_eq!(chunks[0].old_lines.len(), 3);
                assert_eq!(chunks[0].new_lines.len(), 3);
            }
            _ => panic!("Expected UpdateFile"),
        }
    }

    #[test]
    fn test_parse_patch_python_code() {
        let patch = r#"*** Begin Patch
*** Update File: app.py
@@ def hello()
 def hello():
-    return "hi"
+    return "hello"
*** End Patch"#;
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_patch_json_content() {
        let patch = r#"*** Begin Patch
*** Add File: config.json
+{
+  "key": "value",
+  "number": 42
+}
*** End Patch"#;
        let result = parse_patch_text(patch, ParseMode::Strict);
        assert!(result.is_ok());
        let args = result.unwrap();
        match &args.hunks[0] {
            AddFile { contents, .. } => {
                assert!(contents.contains("\"key\": \"value\""));
            }
            _ => panic!("Expected AddFile"),
        }
    }
}
