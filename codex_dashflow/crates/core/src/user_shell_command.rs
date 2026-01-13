//! User shell command formatting
//!
//! This module formats user-initiated shell command executions for the
//! conversation history, making it visible to the model what commands
//! the user ran and their results.

use std::time::Duration;

use crate::context::TruncationPolicy;
use crate::exec::ExecOutput;

/// Opening tag for user shell command records.
pub const USER_SHELL_COMMAND_OPEN: &str = "<user_shell_command>";

/// Closing tag for user shell command records.
pub const USER_SHELL_COMMAND_CLOSE: &str = "</user_shell_command>";

/// Check if text is a user shell command record.
pub fn is_user_shell_command_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    let lowered = trimmed.to_ascii_lowercase();
    lowered.starts_with(USER_SHELL_COMMAND_OPEN)
}

/// Format a duration for display.
fn format_duration_line(duration: Duration) -> String {
    let duration_seconds = duration.as_secs_f64();
    format!("Duration: {duration_seconds:.4} seconds")
}

/// Format the exec output for display.
fn format_exec_output(output: &ExecOutput, policy: TruncationPolicy) -> String {
    use crate::context::truncate_text;

    // Prefer aggregated output if available, otherwise use stdout
    let text = if !output.aggregated_output.text.is_empty() {
        &output.aggregated_output.text
    } else if !output.stdout.text.is_empty() {
        &output.stdout.text
    } else {
        &output.stderr.text
    };

    truncate_text(text, policy)
}

/// Format the body of a user shell command record.
fn format_user_shell_command_body(
    command: &str,
    exec_output: &ExecOutput,
    truncation_policy: TruncationPolicy,
) -> String {
    let mut sections = Vec::new();
    sections.push("<command>".to_string());
    sections.push(command.to_string());
    sections.push("</command>".to_string());
    sections.push("<result>".to_string());
    sections.push(format!("Exit code: {}", exec_output.exit_code));
    sections.push(format_duration_line(exec_output.duration));
    sections.push("Output:".to_string());
    sections.push(format_exec_output(exec_output, truncation_policy));
    sections.push("</result>".to_string());
    sections.join("\n")
}

/// Format a complete user shell command record.
pub fn format_user_shell_command_record(
    command: &str,
    exec_output: &ExecOutput,
    truncation_policy: TruncationPolicy,
) -> String {
    let body = format_user_shell_command_body(command, exec_output, truncation_policy);
    format!("{USER_SHELL_COMMAND_OPEN}\n{body}\n{USER_SHELL_COMMAND_CLOSE}")
}

/// A user shell command record that can be added to conversation history.
#[derive(Debug, Clone)]
pub struct UserShellCommandRecord {
    /// The command that was executed
    pub command: String,
    /// The formatted record text
    pub text: String,
    /// Exit code from execution
    pub exit_code: i32,
    /// Duration of execution
    pub duration: Duration,
    /// Whether the command timed out
    pub timed_out: bool,
}

impl UserShellCommandRecord {
    /// Create a new user shell command record.
    pub fn new(
        command: impl Into<String>,
        exec_output: &ExecOutput,
        truncation_policy: TruncationPolicy,
    ) -> Self {
        let command = command.into();
        let text = format_user_shell_command_record(&command, exec_output, truncation_policy);
        Self {
            command,
            text,
            exit_code: exec_output.exit_code,
            duration: exec_output.duration,
            timed_out: exec_output.timed_out,
        }
    }

    /// Check if the command succeeded.
    pub fn success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::StreamOutput;

    fn make_exec_output(
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        aggregated: &str,
        duration: Duration,
    ) -> ExecOutput {
        ExecOutput {
            exit_code,
            stdout: StreamOutput::new(stdout.to_string()),
            stderr: StreamOutput::new(stderr.to_string()),
            aggregated_output: StreamOutput::new(aggregated.to_string()),
            duration,
            timed_out: false,
        }
    }

    #[test]
    fn test_is_user_shell_command_text() {
        assert!(is_user_shell_command_text(
            "<user_shell_command>\necho hi\n</user_shell_command>"
        ));
        assert!(is_user_shell_command_text("  <user_shell_command>"));
        assert!(is_user_shell_command_text("<USER_SHELL_COMMAND>"));
        assert!(!is_user_shell_command_text("echo hi"));
        assert!(!is_user_shell_command_text(
            "some text <user_shell_command>"
        ));
    }

    #[test]
    fn test_format_duration_line() {
        assert_eq!(
            format_duration_line(Duration::from_secs(1)),
            "Duration: 1.0000 seconds"
        );
        assert_eq!(
            format_duration_line(Duration::from_millis(120)),
            "Duration: 0.1200 seconds"
        );
        assert_eq!(
            format_duration_line(Duration::from_millis(1500)),
            "Duration: 1.5000 seconds"
        );
    }

    #[test]
    fn test_format_user_shell_command_record_basic() {
        let exec_output = make_exec_output(0, "hi", "", "hi", Duration::from_secs(1));
        let record = format_user_shell_command_record(
            "echo hi",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );

        assert_eq!(
            record,
            "<user_shell_command>\n<command>\necho hi\n</command>\n<result>\nExit code: 0\nDuration: 1.0000 seconds\nOutput:\nhi\n</result>\n</user_shell_command>"
        );
    }

    #[test]
    fn test_format_user_shell_command_uses_aggregated_output() {
        let exec_output = make_exec_output(
            42,
            "stdout-only",
            "stderr-only",
            "combined output wins",
            Duration::from_millis(120),
        );
        let record = format_user_shell_command_record(
            "false",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );

        assert_eq!(
            record,
            "<user_shell_command>\n<command>\nfalse\n</command>\n<result>\nExit code: 42\nDuration: 0.1200 seconds\nOutput:\ncombined output wins\n</result>\n</user_shell_command>"
        );
    }

    #[test]
    fn test_user_shell_command_record_new() {
        let exec_output = make_exec_output(0, "output", "", "output", Duration::from_secs(2));
        let record =
            UserShellCommandRecord::new("ls -la", &exec_output, TruncationPolicy::Bytes(10_000));

        assert_eq!(record.command, "ls -la");
        assert_eq!(record.exit_code, 0);
        assert_eq!(record.duration, Duration::from_secs(2));
        assert!(!record.timed_out);
        assert!(record.success());
        assert!(record.text.contains("<user_shell_command>"));
        assert!(record.text.contains("ls -la"));
    }

    #[test]
    fn test_user_shell_command_record_failed() {
        let exec_output = ExecOutput {
            exit_code: 1,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new("error".to_string()),
            aggregated_output: StreamOutput::new("error".to_string()),
            duration: Duration::from_millis(100),
            timed_out: false,
        };
        let record = UserShellCommandRecord::new(
            "bad_command",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );

        assert!(!record.success());
        assert_eq!(record.exit_code, 1);
    }

    #[test]
    fn test_user_shell_command_record_timed_out() {
        let exec_output = ExecOutput {
            exit_code: 124,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::from_secs(30),
            timed_out: true,
        };
        let record =
            UserShellCommandRecord::new("sleep 100", &exec_output, TruncationPolicy::Bytes(10_000));

        assert!(!record.success());
        assert!(record.timed_out);
    }

    #[test]
    fn test_format_exec_output_prefers_aggregated() {
        let output = make_exec_output(0, "stdout", "stderr", "aggregated", Duration::ZERO);
        let formatted = format_exec_output(&output, TruncationPolicy::Bytes(10_000));
        assert_eq!(formatted, "aggregated");
    }

    #[test]
    fn test_format_exec_output_falls_back_to_stdout() {
        let output = make_exec_output(0, "stdout", "stderr", "", Duration::ZERO);
        let formatted = format_exec_output(&output, TruncationPolicy::Bytes(10_000));
        assert_eq!(formatted, "stdout");
    }

    #[test]
    fn test_format_exec_output_falls_back_to_stderr() {
        let output = make_exec_output(0, "", "stderr", "", Duration::ZERO);
        let formatted = format_exec_output(&output, TruncationPolicy::Bytes(10_000));
        assert_eq!(formatted, "stderr");
    }

    #[test]
    fn test_constants() {
        assert_eq!(USER_SHELL_COMMAND_OPEN, "<user_shell_command>");
        assert_eq!(USER_SHELL_COMMAND_CLOSE, "</user_shell_command>");
    }

    #[test]
    fn test_is_user_shell_command_text_case_variations() {
        // Mixed case
        assert!(is_user_shell_command_text("<User_Shell_Command>"));
        assert!(is_user_shell_command_text("<uSeR_sHeLl_cOmMaNd>"));
    }

    #[test]
    fn test_is_user_shell_command_text_with_tabs() {
        assert!(is_user_shell_command_text("\t<user_shell_command>"));
        assert!(is_user_shell_command_text("\t\t<user_shell_command>"));
    }

    #[test]
    fn test_is_user_shell_command_text_empty() {
        assert!(!is_user_shell_command_text(""));
        assert!(!is_user_shell_command_text("   "));
        assert!(!is_user_shell_command_text("\n"));
    }

    #[test]
    fn test_format_duration_line_zero() {
        assert_eq!(
            format_duration_line(Duration::ZERO),
            "Duration: 0.0000 seconds"
        );
    }

    #[test]
    fn test_format_duration_line_microseconds() {
        assert_eq!(
            format_duration_line(Duration::from_micros(500)),
            "Duration: 0.0005 seconds"
        );
    }

    #[test]
    fn test_format_duration_line_large() {
        assert_eq!(
            format_duration_line(Duration::from_secs(3600)),
            "Duration: 3600.0000 seconds"
        );
    }

    #[test]
    fn test_user_shell_command_record_debug() {
        let exec_output = make_exec_output(0, "out", "", "out", Duration::from_secs(1));
        let record =
            UserShellCommandRecord::new("echo test", &exec_output, TruncationPolicy::Bytes(1000));
        let debug_str = format!("{:?}", record);
        assert!(debug_str.contains("UserShellCommandRecord"));
        assert!(debug_str.contains("command"));
    }

    #[test]
    fn test_user_shell_command_record_clone() {
        let exec_output = make_exec_output(0, "out", "", "out", Duration::from_secs(1));
        let record =
            UserShellCommandRecord::new("echo test", &exec_output, TruncationPolicy::Bytes(1000));
        let cloned = record.clone();
        assert_eq!(record.command, cloned.command);
        assert_eq!(record.exit_code, cloned.exit_code);
        assert_eq!(record.duration, cloned.duration);
        assert_eq!(record.text, cloned.text);
    }

    #[test]
    fn test_format_exec_output_all_empty() {
        let output = make_exec_output(0, "", "", "", Duration::ZERO);
        let formatted = format_exec_output(&output, TruncationPolicy::Bytes(10_000));
        assert_eq!(formatted, "");
    }

    #[test]
    fn test_format_user_shell_command_body_structure() {
        let exec_output = make_exec_output(0, "", "", "output", Duration::from_millis(500));
        let body =
            format_user_shell_command_body("test cmd", &exec_output, TruncationPolicy::Bytes(1000));

        assert!(body.starts_with("<command>"));
        assert!(body.contains("test cmd"));
        assert!(body.contains("</command>"));
        assert!(body.contains("<result>"));
        assert!(body.contains("Exit code: 0"));
        assert!(body.contains("Duration:"));
        assert!(body.contains("Output:"));
        assert!(body.contains("</result>"));
    }

    #[test]
    fn test_record_with_multiline_command() {
        let exec_output = make_exec_output(0, "out", "", "out", Duration::from_secs(1));
        let record = UserShellCommandRecord::new(
            "echo line1\necho line2\necho line3",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );
        assert!(record.text.contains("line1\necho line2\necho line3"));
    }

    #[test]
    fn test_record_with_special_characters_in_command() {
        let exec_output = make_exec_output(0, "out", "", "out", Duration::from_secs(1));
        let record = UserShellCommandRecord::new(
            "echo \"hello\" | grep 'world' && ls -la",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );
        assert!(record.text.contains("echo \"hello\""));
        assert!(record.text.contains("grep 'world'"));
    }

    #[test]
    fn test_record_negative_exit_code() {
        let exec_output = ExecOutput {
            exit_code: -1,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new("signal".to_string()),
            aggregated_output: StreamOutput::new("signal".to_string()),
            duration: Duration::from_millis(100),
            timed_out: false,
        };
        let record = UserShellCommandRecord::new(
            "kill -9 $$",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );
        assert!(!record.success());
        assert_eq!(record.exit_code, -1);
    }

    #[test]
    fn test_success_requires_zero_exit_and_no_timeout() {
        // Zero exit but timed out
        let exec_output = ExecOutput {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::from_secs(30),
            timed_out: true,
        };
        let record =
            UserShellCommandRecord::new("cmd", &exec_output, TruncationPolicy::Bytes(10_000));
        assert!(!record.success()); // Should fail even with exit code 0
    }

    #[test]
    fn test_format_record_contains_all_parts() {
        let exec_output =
            make_exec_output(127, "stdout", "stderr", "combined", Duration::from_secs(5));
        let record = format_user_shell_command_record(
            "my_cmd",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );

        assert!(record.starts_with(USER_SHELL_COMMAND_OPEN));
        assert!(record.ends_with(USER_SHELL_COMMAND_CLOSE));
        assert!(record.contains("<command>"));
        assert!(record.contains("my_cmd"));
        assert!(record.contains("</command>"));
        assert!(record.contains("<result>"));
        assert!(record.contains("Exit code: 127"));
        assert!(record.contains("Duration: 5.0000 seconds"));
        assert!(record.contains("Output:"));
        assert!(record.contains("combined"));
        assert!(record.contains("</result>"));
    }

    #[test]
    fn test_format_exec_output_with_truncation() {
        let long_output = "x".repeat(10000);
        let output = make_exec_output(0, "", "", &long_output, Duration::ZERO);
        let formatted = format_exec_output(&output, TruncationPolicy::Bytes(100));
        // Should be truncated
        assert!(formatted.len() < long_output.len());
    }

    #[test]
    fn test_record_unicode_command() {
        let exec_output = make_exec_output(0, "日本語", "", "日本語", Duration::from_secs(1));
        let record = UserShellCommandRecord::new(
            "echo 你好世界",
            &exec_output,
            TruncationPolicy::Bytes(10_000),
        );
        assert!(record.text.contains("你好世界"));
        assert!(record.text.contains("日本語"));
    }

    #[test]
    fn test_record_empty_command() {
        let exec_output = make_exec_output(0, "", "", "", Duration::ZERO);
        let record = UserShellCommandRecord::new("", &exec_output, TruncationPolicy::Bytes(10_000));
        assert_eq!(record.command, "");
        assert!(record.text.contains("<command>\n\n</command>"));
    }
}
