//! Skill rendering for system prompts

use super::model::SkillMetadata;

/// Render a skills section suitable for including in a system prompt.
///
/// Returns `None` if the skills list is empty.
///
/// # Format
///
/// The output includes:
/// - A header and explanation
/// - A bulleted list of skills with name, description, and file path
/// - Instructions for how to use skills
pub fn render_skills_section(skills: &[SkillMetadata]) -> Option<String> {
    if skills.is_empty() {
        return None;
    }

    let mut lines: Vec<String> = Vec::new();

    lines.push("## Skills".to_string());
    lines.push(
        "These skills are discovered at startup from ~/.codex-dashflow/skills; each entry shows \
         name, description, and file path so you can open the source for full instructions. \
         Content is not inlined to keep context lean."
            .to_string(),
    );

    // List each skill
    for skill in skills {
        let path_str = skill.path.to_string_lossy().replace('\\', "/");
        lines.push(format!(
            "- {}: {} (file: {})",
            skill.name, skill.description, path_str
        ));
    }

    // Usage instructions
    lines.push(SKILL_USAGE_INSTRUCTIONS.to_string());

    Some(lines.join("\n"))
}

/// Instructions for how the LLM should use skills.
const SKILL_USAGE_INSTRUCTIONS: &str = r###"- Discovery: Available skills are listed in project docs and may also appear in a runtime "## Skills" section (name + description + file path). These are the sources of truth; skill bodies live on disk at the listed paths.
- Trigger rules: If the user names a skill (with `$SkillName` or plain text) OR the task clearly matches a skill's description, you must use that skill for that turn. Multiple mentions mean use them all. Do not carry skills across turns unless re-mentioned.
- Missing/blocked: If a named skill isn't in the list or the path can't be read, say so briefly and continue with the best fallback.
- How to use a skill (progressive disclosure):
  1) After deciding to use a skill, open its `SKILL.md`. Read only enough to follow the workflow.
  2) If `SKILL.md` points to extra folders such as `references/`, load only the specific files needed for the request; don't bulk-load everything.
  3) If `scripts/` exist, prefer running or patching them instead of retyping large code blocks.
  4) If `assets/` or templates exist, reuse them instead of recreating from scratch.
- Description as trigger: The YAML `description` in `SKILL.md` is the primary trigger signal; rely on it to decide applicability. If unsure, ask a brief clarification before proceeding.
- Coordination and sequencing:
  - If multiple skills apply, choose the minimal set that covers the request and state the order you'll use them.
  - Announce which skill(s) you're using and why (one short line). If you skip an obvious skill, say why.
- Context hygiene:
  - Keep context small: summarize long sections instead of pasting them; only load extra files when needed.
  - Avoid deeply nested references; prefer one-hop files explicitly linked from `SKILL.md`.
  - When variants exist (frameworks, providers, domains), pick only the relevant reference file(s) and note that choice.
- Safety and fallback: If a skill can't be applied cleanly (missing files, unclear instructions), state the issue, pick the next-best approach, and continue."###;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_render_skills_section_empty() {
        assert!(render_skills_section(&[]).is_none());
    }

    #[test]
    fn test_render_skills_section_single() {
        let skills = vec![SkillMetadata {
            name: "code-review".to_string(),
            description: "Reviews code for bugs".to_string(),
            path: PathBuf::from("/home/user/.codex-dashflow/skills/code-review/SKILL.md"),
        }];

        let result = render_skills_section(&skills).unwrap();

        assert!(result.contains("## Skills"));
        assert!(result.contains("code-review: Reviews code for bugs"));
        assert!(result.contains("file:"));
        assert!(result.contains("Trigger rules"));
    }

    #[test]
    fn test_render_skills_section_multiple() {
        let skills = vec![
            SkillMetadata {
                name: "skill-a".to_string(),
                description: "Does A".to_string(),
                path: PathBuf::from("/skills/a/SKILL.md"),
            },
            SkillMetadata {
                name: "skill-b".to_string(),
                description: "Does B".to_string(),
                path: PathBuf::from("/skills/b/SKILL.md"),
            },
        ];

        let result = render_skills_section(&skills).unwrap();

        assert!(result.contains("skill-a: Does A"));
        assert!(result.contains("skill-b: Does B"));
    }

    #[test]
    fn test_render_skills_section_windows_path_normalization() {
        let skills = vec![SkillMetadata {
            name: "test".to_string(),
            description: "Test skill".to_string(),
            path: PathBuf::from("C:\\Users\\test\\skills\\SKILL.md"),
        }];

        let result = render_skills_section(&skills).unwrap();

        // Backslashes should be converted to forward slashes
        assert!(result.contains("C:/Users/test/skills/SKILL.md"));
        assert!(!result.contains('\\'));
    }
}
