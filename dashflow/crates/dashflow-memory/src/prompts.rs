//! Prompts used by memory implementations

use dashflow::core::prompts::base::PromptTemplateFormat;
use dashflow::core::prompts::string::PromptTemplate;

/// Default prompt template string for conversation summarization
pub const SUMMARY_PROMPT_TEMPLATE: &str = r"Progressively summarize the lines of conversation provided, adding onto the previous summary returning a new summary.

EXAMPLE
Current summary:
The human asks what the AI thinks of artificial intelligence. The AI thinks artificial intelligence is a force for good.

New lines of conversation:
Human: Why do you think artificial intelligence is a force for good?
AI: Because artificial intelligence will help humans reach their full potential.

New summary:
The human asks what the AI thinks of artificial intelligence. The AI thinks artificial intelligence is a force for good because it will help humans reach their full potential.
END OF EXAMPLE

Current summary:
{summary}

New lines of conversation:
{new_lines}

New summary:";

/// Create the default summarization prompt template.
///
/// This prompt progressively summarizes conversation lines, building upon
/// the existing summary with new information. The prompt guides the LLM to:
/// - Add new information from recent conversation turns
/// - Maintain context from the existing summary
/// - Return a concise, progressive summary
///
/// # Input Variables
///
/// - `summary`: The existing summary to build upon (empty string if first summary)
/// - `new_lines`: New conversation lines to add to the summary
///
/// # Python Baseline
///
/// Matches `SUMMARY_PROMPT` from `dashflow.memory.prompt:46-48`
#[must_use]
pub fn create_summary_prompt() -> PromptTemplate {
    PromptTemplate::new(
        SUMMARY_PROMPT_TEMPLATE,
        vec!["summary".to_string(), "new_lines".to_string()],
        PromptTemplateFormat::FString,
    )
}

/// Convenience constant for backward compatibility
pub const SUMMARY_PROMPT: fn() -> PromptTemplate = create_summary_prompt;

/// Default prompt template string for entity extraction
pub const ENTITY_EXTRACTION_PROMPT_TEMPLATE: &str = r#"You are an AI assistant reading the transcript of a conversation between an AI and a human. Extract all of the proper nouns from the last line of conversation. As a guideline, a proper noun is generally capitalized. You should definitely extract all names and places.

The conversation history is provided just in case of a coreference (e.g. "What do you know about him" where "him" is defined in a previous line) -- ignore items mentioned there that are not in the last line.

Return the output as a single comma-separated list, or NONE if there is nothing of note to return (e.g. the user is just issuing a greeting or having a simple conversation).

EXAMPLE
Conversation history:
Person #1: how's it going today?
AI: "It's going great! How about you?"
Person #1: good! busy working on Langchain. lots to do.
AI: "That sounds like a lot of work! What kind of things are you doing to make Langchain better?"
Last line:
Person #1: i'm trying to improve Langchain's interfaces, the UX, its integrations with various products the user might want ... a lot of stuff.
Output: Langchain
END OF EXAMPLE

EXAMPLE
Conversation history:
Person #1: how's it going today?
AI: "It's going great! How about you?"
Person #1: good! busy working on Langchain. lots to do.
AI: "That sounds like a lot of work! What kind of things are you doing to make Langchain better?"
Last line:
Person #1: i'm trying to improve Langchain's interfaces, the UX, its integrations with various products the user might want ... a lot of stuff. I'm working with Person #2.
Output: Langchain, Person #2
END OF EXAMPLE

Conversation history (for reference only):
{history}
Last line of conversation (for extraction):
Human: {input}

Output:"#;

/// Create the default entity extraction prompt template.
///
/// This prompt extracts proper nouns (names, places, entities) from the most
/// recent line of conversation. The LLM is instructed to:
/// - Focus on the last line of conversation only
/// - Extract capitalized proper nouns (names, places, organizations)
/// - Return a comma-separated list or "NONE"
/// - Use conversation history only for coreference resolution
///
/// # Input Variables
///
/// - `history`: Full conversation history for context/coreference
/// - `input`: The last line from the human to extract entities from
///
/// # Python Baseline
///
/// Matches `ENTITY_EXTRACTION_PROMPT` from `dashflow.memory.prompt:84-86`
#[must_use]
pub fn create_entity_extraction_prompt() -> PromptTemplate {
    PromptTemplate::new(
        ENTITY_EXTRACTION_PROMPT_TEMPLATE,
        vec!["history".to_string(), "input".to_string()],
        PromptTemplateFormat::FString,
    )
}

/// Convenience constant for entity extraction prompt
pub const ENTITY_EXTRACTION_PROMPT: fn() -> PromptTemplate = create_entity_extraction_prompt;

/// Default prompt template string for entity summarization
pub const ENTITY_SUMMARIZATION_PROMPT_TEMPLATE: &str = r#"You are an AI assistant helping a human keep track of facts about relevant people, places, and concepts in their life. Update the summary of the provided entity in the "Entity" section based on the last line of your conversation with the human. If you are writing the summary for the first time, return a single sentence.
The update should only include facts that are relayed in the last line of conversation about the provided entity, and should only contain facts about the provided entity.

If there is no new information about the provided entity or the information is not worth noting (not an important or relevant fact to remember long-term), return the existing summary unchanged.

Full conversation history (for context):
{history}

Entity to summarize:
{entity}

Existing summary of {entity}:
{summary}

Last line of conversation:
Human: {input}
Updated summary:"#;

/// Create the default entity summarization prompt template.
///
/// This prompt updates summaries for specific entities based on new information
/// in the conversation. The LLM is instructed to:
/// - Update only with facts from the last conversation line
/// - Focus on the specific entity being summarized
/// - Return existing summary unchanged if no new relevant information
/// - Create a single sentence summary if this is the first summary
///
/// # Input Variables
///
/// - `entity`: Name of the entity to summarize
/// - `summary`: Existing summary of the entity (empty string if first time)
/// - `history`: Full conversation history for context
/// - `input`: The last line from the human containing entity information
///
/// # Python Baseline
///
/// Matches `ENTITY_SUMMARIZATION_PROMPT` from `dashflow.memory.prompt:106-109`
#[must_use]
pub fn create_entity_summarization_prompt() -> PromptTemplate {
    PromptTemplate::new(
        ENTITY_SUMMARIZATION_PROMPT_TEMPLATE,
        vec![
            "entity".to_string(),
            "summary".to_string(),
            "history".to_string(),
            "input".to_string(),
        ],
        PromptTemplateFormat::FString,
    )
}

/// Convenience constant for entity summarization prompt
pub const ENTITY_SUMMARIZATION_PROMPT: fn() -> PromptTemplate = create_entity_summarization_prompt;

/// Delimiter used to separate knowledge triples in extraction output
pub const KG_TRIPLE_DELIMITER: &str = "<|>";

/// Default prompt template string for knowledge triple extraction
pub const KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE: &str = r"You are a networked intelligence helping a human track knowledge triples about all relevant people, things, concepts, etc. and integrating them with your knowledge stored within your weights as well as that stored in a knowledge graph. Extract all of the knowledge triples from the last line of conversation. A knowledge triple is a clause that contains a subject, a predicate, and an object. The subject is the entity being described, the predicate is the property of the subject that is being described, and the object is the value of the property.

EXAMPLE
Conversation history:
Person #1: Did you hear aliens landed in Area 51?
AI: No, I didn't hear that. What do you know about Area 51?
Person #1: It's a secret military base in Nevada.
AI: What do you know about Nevada?
Last line of conversation:
Person #1: It's a state in the US. It's also the number 1 producer of gold in the US.

Output: (Nevada, is a, state)<|>(Nevada, is in, US)<|>(Nevada, is the number 1 producer of, gold)
END OF EXAMPLE

EXAMPLE
Conversation history:
Person #1: Hello.
AI: Hi! How are you?
Person #1: I'm good. How are you?
AI: I'm good too.
Last line of conversation:
Person #1: I'm going to the store.

Output: NONE
END OF EXAMPLE

EXAMPLE
Conversation history:
Person #1: What do you know about Descartes?
AI: Descartes was a French philosopher, mathematician, and scientist who lived in the 17th century.
Person #1: The Descartes I'm referring to is a standup comedian and interior designer from Montreal.
AI: Oh yes, He is a comedian and an interior designer. He has been in the industry for 30 years. His favorite food is baked bean pie.
Last line of conversation:
Person #1: Oh huh. I know Descartes likes to drive antique scooters and play the mandolin.
Output: (Descartes, likes to drive, antique scooters)<|>(Descartes, plays, mandolin)
END OF EXAMPLE

Conversation history (for reference only):
{history}
Last line of conversation (for extraction):
Human: {input}

Output:";

/// Create the default knowledge triple extraction prompt template.
///
/// This prompt extracts structured knowledge triples (subject, predicate, object)
/// from the most recent line of conversation. The LLM is instructed to:
/// - Focus on the last line of conversation only
/// - Extract triples in the format: (subject, predicate, object)
/// - Separate multiple triples with the `KG_TRIPLE_DELIMITER` ("<|>")
/// - Return "NONE" if no meaningful knowledge can be extracted
/// - Use conversation history only for context
///
/// # Input Variables
///
/// - `history`: Full conversation history for context
/// - `input`: The last line from the human to extract knowledge from
///
/// # Python Baseline
///
/// Matches `KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT` from `dashflow.memory.prompt:161-164`
#[must_use]
pub fn create_knowledge_triple_extraction_prompt() -> PromptTemplate {
    PromptTemplate::new(
        KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE,
        vec!["history".to_string(), "input".to_string()],
        PromptTemplateFormat::FString,
    )
}

/// Convenience constant for knowledge triple extraction prompt
pub const KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT: fn() -> PromptTemplate =
    create_knowledge_triple_extraction_prompt;

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::prompts::BasePromptTemplate;

    // ============================================
    // Summary Prompt Tests
    // ============================================

    #[test]
    fn test_summary_prompt_template_constant_not_empty() {
        assert_ne!(SUMMARY_PROMPT_TEMPLATE, "");
    }

    #[test]
    fn test_summary_prompt_template_contains_variables() {
        assert!(SUMMARY_PROMPT_TEMPLATE.contains("{summary}"));
        assert!(SUMMARY_PROMPT_TEMPLATE.contains("{new_lines}"));
    }

    #[test]
    fn test_create_summary_prompt_returns_template() {
        let template = create_summary_prompt();
        assert_eq!(
            template.input_variables(),
            &["summary".to_string(), "new_lines".to_string()]
        );
    }

    #[test]
    fn test_summary_prompt_constant_function_works() {
        let template = SUMMARY_PROMPT();
        assert_eq!(
            template.input_variables(),
            &["summary".to_string(), "new_lines".to_string()]
        );
    }

    #[test]
    fn test_summary_prompt_template_has_example() {
        assert!(SUMMARY_PROMPT_TEMPLATE.contains("EXAMPLE"));
        assert!(SUMMARY_PROMPT_TEMPLATE.contains("END OF EXAMPLE"));
    }

    #[test]
    fn test_summary_prompt_ends_with_new_summary() {
        assert!(SUMMARY_PROMPT_TEMPLATE.ends_with("New summary:"));
    }

    // ============================================
    // Entity Extraction Prompt Tests
    // ============================================

    #[test]
    fn test_entity_extraction_prompt_template_not_empty() {
        assert_ne!(ENTITY_EXTRACTION_PROMPT_TEMPLATE, "");
    }

    #[test]
    fn test_entity_extraction_prompt_template_contains_variables() {
        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.contains("{history}"));
        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.contains("{input}"));
    }

    #[test]
    fn test_create_entity_extraction_prompt_returns_template() {
        let template = create_entity_extraction_prompt();
        assert_eq!(
            template.input_variables(),
            &["history".to_string(), "input".to_string()]
        );
    }

    #[test]
    fn test_entity_extraction_prompt_constant_function_works() {
        let template = ENTITY_EXTRACTION_PROMPT();
        assert_eq!(
            template.input_variables(),
            &["history".to_string(), "input".to_string()]
        );
    }

    #[test]
    fn test_entity_extraction_prompt_has_examples() {
        // Should have multiple examples for clarity
        let example_count = ENTITY_EXTRACTION_PROMPT_TEMPLATE.matches("EXAMPLE").count();
        assert!(example_count >= 4); // 2 EXAMPLE + 2 END OF EXAMPLE
    }

    #[test]
    fn test_entity_extraction_prompt_mentions_none_output() {
        // Should mention NONE as a valid output
        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.contains("NONE"));
    }

    #[test]
    fn test_entity_extraction_prompt_ends_with_output() {
        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.ends_with("Output:"));
    }

    // ============================================
    // Entity Summarization Prompt Tests
    // ============================================

    #[test]
    fn test_entity_summarization_prompt_template_not_empty() {
        assert_ne!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE, "");
    }

    #[test]
    fn test_entity_summarization_prompt_template_contains_variables() {
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.contains("{entity}"));
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.contains("{summary}"));
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.contains("{history}"));
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.contains("{input}"));
    }

    #[test]
    fn test_create_entity_summarization_prompt_returns_template() {
        let template = create_entity_summarization_prompt();
        assert_eq!(
            template.input_variables(),
            &[
                "entity".to_string(),
                "summary".to_string(),
                "history".to_string(),
                "input".to_string()
            ]
        );
    }

    #[test]
    fn test_entity_summarization_prompt_constant_function_works() {
        let template = ENTITY_SUMMARIZATION_PROMPT();
        assert_eq!(
            template.input_variables(),
            &[
                "entity".to_string(),
                "summary".to_string(),
                "history".to_string(),
                "input".to_string()
            ]
        );
    }

    #[test]
    fn test_entity_summarization_prompt_has_four_variables() {
        let template = create_entity_summarization_prompt();
        assert_eq!(template.input_variables().len(), 4);
    }

    #[test]
    fn test_entity_summarization_prompt_ends_with_updated_summary() {
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.ends_with("Updated summary:"));
    }

    // ============================================
    // Knowledge Triple Extraction Prompt Tests
    // ============================================

    #[test]
    fn test_kg_triple_delimiter_value() {
        assert_eq!(KG_TRIPLE_DELIMITER, "<|>");
    }

    #[test]
    fn test_kg_triple_delimiter_not_empty() {
        assert_ne!(KG_TRIPLE_DELIMITER, "");
    }

    #[test]
    fn test_kg_triple_delimiter_length() {
        assert_eq!(KG_TRIPLE_DELIMITER.len(), 3);
    }

    #[test]
    fn test_knowledge_triple_prompt_template_not_empty() {
        assert_ne!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE, "");
    }

    #[test]
    fn test_knowledge_triple_prompt_template_contains_variables() {
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.contains("{history}"));
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.contains("{input}"));
    }

    #[test]
    fn test_create_knowledge_triple_extraction_prompt_returns_template() {
        let template = create_knowledge_triple_extraction_prompt();
        assert_eq!(
            template.input_variables(),
            &["history".to_string(), "input".to_string()]
        );
    }

    #[test]
    fn test_knowledge_triple_prompt_constant_function_works() {
        let template = KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT();
        assert_eq!(
            template.input_variables(),
            &["history".to_string(), "input".to_string()]
        );
    }

    #[test]
    fn test_knowledge_triple_prompt_uses_delimiter_format() {
        // The template should demonstrate the delimiter format
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.contains(KG_TRIPLE_DELIMITER));
    }

    #[test]
    fn test_knowledge_triple_prompt_has_examples() {
        // Should have multiple examples
        let example_count = KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE
            .matches("EXAMPLE")
            .count();
        assert!(example_count >= 6); // 3 EXAMPLE + 3 END OF EXAMPLE
    }

    #[test]
    fn test_knowledge_triple_prompt_shows_none_output() {
        // Should show NONE as a valid output when no triples can be extracted
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.contains("Output: NONE"));
    }

    #[test]
    fn test_knowledge_triple_prompt_ends_with_output() {
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.ends_with("Output:"));
    }

    // ============================================
    // Cross-Prompt Consistency Tests
    // ============================================

    #[test]
    fn test_all_prompts_use_fstring_format() {
        // All prompts should use FString format for consistency
        let summary = create_summary_prompt();
        let entity_extraction = create_entity_extraction_prompt();
        let entity_summarization = create_entity_summarization_prompt();
        let knowledge_triple = create_knowledge_triple_extraction_prompt();

        // Verify they're all using curly brace variable format
        assert!(SUMMARY_PROMPT_TEMPLATE.contains("{"));
        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.contains("{"));
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.contains("{"));
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.contains("{"));

        // All should have at least 2 input variables
        assert!(summary.input_variables().len() >= 2);
        assert!(entity_extraction.input_variables().len() >= 2);
        assert!(entity_summarization.input_variables().len() >= 2);
        assert!(knowledge_triple.input_variables().len() >= 2);
    }

    #[test]
    fn test_prompts_have_reasonable_lengths() {
        // Prompts shouldn't be too short (incomplete) or too long (inefficient)
        assert!(SUMMARY_PROMPT_TEMPLATE.len() > 200);
        assert!(SUMMARY_PROMPT_TEMPLATE.len() < 5000);

        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.len() > 500);
        assert!(ENTITY_EXTRACTION_PROMPT_TEMPLATE.len() < 5000);

        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.len() > 200);
        assert!(ENTITY_SUMMARIZATION_PROMPT_TEMPLATE.len() < 5000);

        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.len() > 500);
        assert!(KNOWLEDGE_TRIPLE_EXTRACTION_PROMPT_TEMPLATE.len() < 5000);
    }
}
