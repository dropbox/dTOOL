//! # Constitutional AI Chain
//!
//! Implements the Constitutional AI self-critique method from (Bai et al., 2022).
//! See: <https://arxiv.org/pdf/2212.08073.pdf>
//!
//! Constitutional AI is a method for training AI systems to be helpful, harmless, and honest
//! by having them critique and revise their own outputs according to constitutional principles.
//!
//! This chain:
//! 1. Runs an initial LLM chain to generate a response
//! 2. For each constitutional principle:
//!    - Critiques the response according to the principle
//!    - If critique indicates issues, revises the response
//! 3. Returns the final (revised) response
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_chains::{ConstitutionalChain, ConstitutionalPrinciple, LLMChain};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Create an LLM chain
//!     // let llm = ...;
//!     // let prompt = ...;
//!     // let initial_chain = LLMChain::new(llm.clone(), prompt);
//!
//!     // Define constitutional principles
//!     let principles = vec![
//!         ConstitutionalPrinciple::harmful1(),  // Built-in principle
//!         ConstitutionalPrinciple::new(
//!             "custom",
//!             "Is this response factual?",
//!             "Please rewrite to be more factual.",
//!         ),
//!     ];
//!
//!     // Create constitutional chain
//!     // let chain = ConstitutionalChain::from_llm(
//!     //     llm,
//!     //     initial_chain,
//!     //     principles,
//!     // );
//!
//!     // Run the chain
//!     // let mut inputs = HashMap::new();
//!     // inputs.insert("question".to_string(), "What is AI?".to_string());
//!     // let result = chain.call(&inputs).await.unwrap();
//! }
//! ```

use crate::LLMChain;
use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::LLM;
use dashflow::core::prompts::PromptTemplate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// A constitutional principle for critiquing and revising AI outputs.
///
/// Each principle defines:
/// - A critique request: how to evaluate the response
/// - A revision request: how to improve it if issues are found
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionalPrinciple {
    /// Name/identifier for this principle
    pub name: String,

    /// Prompt for critiquing the response
    /// Example: "Identify ways this response is harmful or unethical"
    pub critique_request: String,

    /// Prompt for revising the response if critique finds issues
    /// Example: "Rewrite to remove harmful content"
    pub revision_request: String,
}

impl ConstitutionalPrinciple {
    /// Create a new constitutional principle.
    ///
    /// # Arguments
    ///
    /// * `name` - Identifier for this principle
    /// * `critique_request` - How to evaluate the response
    /// * `revision_request` - How to improve if issues found
    pub fn new(
        name: impl Into<String>,
        critique_request: impl Into<String>,
        revision_request: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            critique_request: critique_request.into(),
            revision_request: revision_request.into(),
        }
    }

    // Built-in principles from Bai et al. 2022

    /// Identify harmful, unethical, racist, sexist, toxic, dangerous, or illegal content.
    #[must_use]
    pub fn harmful1() -> Self {
        Self::new(
            "harmful1",
            "Identify specific ways in which the assistant's last response is harmful, unethical, racist, sexist, toxic, dangerous, or illegal.",
            "Please rewrite the assistant response to remove any and all harmful, unethical, racist, sexist, toxic, dangerous, or illegal content.",
        )
    }

    /// Point out harmful content including dangerous or illegal activity.
    #[must_use]
    pub fn harmful2() -> Self {
        Self::new(
            "harmful2",
            "Explain ways in which the assistant's preceding response may be harmful to either the human or others. For example, if the assistant says anything that encourages dangerous or illegal activity, make sure to point it out. Other examples of harm include racism, sexism, and other social biases.",
            "Please rewrite the assistant response to remove any and all harmful content, including anything dangerous, illegal, racist, sexist or socially biased.",
        )
    }

    /// Identify harmful, unethical, or socially biased content with improvement suggestions.
    #[must_use]
    pub fn harmful3() -> Self {
        Self::new(
            "harmful3",
            "Identify all ways in which the assistant's last response is harmful, unethical, or socially biased. Furthermore, provide specific details on how the assistant can improve its response.",
            "Please rewrite the assistant response to remove all harmful, unethical, or socially biased content, and move the conversation in a positive direction.",
        )
    }

    /// Point out harmful assumptions in both human questions and assistant responses.
    #[must_use]
    pub fn harmful4() -> Self {
        Self::new(
            "harmful4",
            "Identify specific ways in which the assistant's precedings response is harmful. Also point out potential harmfulness in the human's questions and responses. Examples of harm include but are not limited to racism, sexism, toxicity.",
            "Rewrite the assistant response to remove all harmful content, and to politely point out any harmful assumptions from the human, such as racism, sexism, toxicity.",
        )
    }

    /// Check for insensitive, sexist, racist, or socially inappropriate content.
    #[must_use]
    pub fn insensitive() -> Self {
        Self::new(
            "insensitive",
            "Discuss the assistant's last response to the human and whether it is in any way insensitive, sexist, racist, or socially inappropriate.",
            "Revise the assistant's last response to remove content that is in any way insensitive, sexist, racist, or socially inappropriate.",
        )
    }

    /// Check for offensive or harmful content.
    #[must_use]
    pub fn offensive() -> Self {
        Self::new(
            "offensive",
            "Discuss whether the assistant said anything that may be interpreted as offensive or harmful. If possible, also point out any harmful assumptions in the human's dialogue.",
            "Please write a new version of the assistant's response that removes offensive and harmful content, and politely addresses any problematic assumptions from the human.",
        )
    }

    /// Identify illegal or dangerous advice.
    #[must_use]
    pub fn illegal() -> Self {
        Self::new(
            "illegal",
            "Discuss whether the assistant provided any advice that may encourage illegal or dangerous activity from either the human or others.",
            "Revise the assistant's response so as to refrain from giving legally questionable or dangerous advice.",
        )
    }

    /// Check for controversial or objectionable content.
    #[must_use]
    pub fn controversial() -> Self {
        Self::new(
            "controversial",
            "Discuss ways in which the assistant's response may be considered controversial or objectionable based on commonsense ethical and moral standards.",
            "Revise the assistant's response if possible so that it's less controversial and objectionable based on commonsense ethical and moral standards.",
        )
    }

    /// Evaluate thoughtfulness and empathy.
    #[must_use]
    pub fn thoughtful() -> Self {
        Self::new(
            "thoughtful",
            "Please comment on whether the assistant's last response is thoughtful and empathetic. Does it seem like the kind of thing a sensitive friend or therapist might say? Give the strongest argument you can for how it could improve.",
            "Please write a new assistant response so that it is more like what a thoughtful, empathetic, caring, sensitive friend or therapist would say.",
        )
    }

    /// Check for misogynistic or gender-biased content.
    #[must_use]
    pub fn misogynistic() -> Self {
        Self::new(
            "misogynistic",
            "Take a look at the assistant's last response and note if it was in any way misogynistic or otherwise gender biased. Give the strongest argument you can that something the assistant said could be interpreted as misogynistic.",
            "Please write a new response that does not have any trace of misogyny or gender bias.",
        )
    }

    /// Check for criminal advice or assistance.
    #[must_use]
    pub fn criminal() -> Self {
        Self::new(
            "criminal",
            "Read the assistant's last response carefully and identify anything it said that provides advice or assistance in potentially criminal activities, such as violence, theft, hacking, robbery, or any other illegal activity. Consider the strongest argument you could give that the assistant response might encourage crime.",
            "Rewrite the assistant response to avoid assisting the human with criminal plans or activities, such as violence, theft, hacking, robbery, or anything illegal.",
        )
    }

    /// Get all built-in principles.
    #[must_use]
    pub fn all_principles() -> Vec<Self> {
        vec![
            Self::harmful1(),
            Self::harmful2(),
            Self::harmful3(),
            Self::harmful4(),
            Self::insensitive(),
            Self::offensive(),
            Self::illegal(),
            Self::controversial(),
            Self::thoughtful(),
            Self::misogynistic(),
            Self::criminal(),
        ]
    }

    /// Get specific principles by name.
    #[must_use]
    pub fn get_principles(names: &[&str]) -> Vec<Self> {
        let all = Self::all_principles();
        names
            .iter()
            .filter_map(|name| all.iter().find(|p| p.name == *name).cloned())
            .collect()
    }
}

/// Prompt template for critique.
pub const CRITIQUE_PROMPT_TEMPLATE: &str = r"Human: {input_prompt}

Model: {output_from_model}

Critique Request: {critique_request}

Critique:";

/// Create default critique prompt.
pub fn critique_prompt() -> Result<PromptTemplate> {
    PromptTemplate::from_template(CRITIQUE_PROMPT_TEMPLATE)
}

/// Prompt template for revision.
pub const REVISION_PROMPT_TEMPLATE: &str = r"Human: {input_prompt}

Model: {output_from_model}

Critique Request: {critique_request}

Critique: {critique}

Revision Request: {revision_request}

Revision:";

/// Create default revision prompt.
pub fn revision_prompt() -> Result<PromptTemplate> {
    PromptTemplate::from_template(REVISION_PROMPT_TEMPLATE)
}

/// Parse critique output to extract just the critique text.
///
/// Removes any trailing "Revision request:" sections and extra newlines.
#[must_use]
pub fn parse_critique(output: &str) -> String {
    let mut result = output;

    // Remove everything after "Revision request:" if present
    if let Some(idx) = result.find("Revision request:") {
        result = &result[..idx];
    }

    // Take only the first paragraph
    if let Some(idx) = result.find("\n\n") {
        result = &result[..idx];
    }

    result.trim().to_string()
}

/// Constitutional AI chain for self-critique and revision.
///
/// This chain applies constitutional principles to critique and improve LLM outputs.
/// It runs an initial chain, then iteratively critiques and revises the output
/// according to each principle.
///
/// # Type Parameters
///
/// * `M` - The LLM type (must implement LLM trait)
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::{ConstitutionalChain, ConstitutionalPrinciple};
/// # async fn example() {
/// // let chain = ConstitutionalChain::from_llm(llm, initial_chain, principles);
/// // let result = chain.call(&inputs).await.unwrap();
/// # }
/// ```
pub struct ConstitutionalChain<M> {
    /// The initial LLM chain to generate responses
    pub chain: Arc<LLMChain<M>>,

    /// Constitutional principles to apply
    pub constitutional_principles: Vec<ConstitutionalPrinciple>,

    /// LLM chain for generating critiques
    pub critique_chain: Arc<LLMChain<M>>,

    /// LLM chain for generating revisions
    pub revision_chain: Arc<LLMChain<M>>,

    /// Whether to return intermediate steps (critiques and revisions)
    pub return_intermediate_steps: bool,
}

impl<M: LLM> ConstitutionalChain<M> {
    /// Create a new `ConstitutionalChain`.
    ///
    /// # Arguments
    ///
    /// * `chain` - Initial LLM chain to generate responses
    /// * `constitutional_principles` - Principles to apply
    /// * `critique_chain` - Chain for generating critiques
    /// * `revision_chain` - Chain for generating revisions
    #[must_use]
    pub fn new(
        chain: LLMChain<M>,
        constitutional_principles: Vec<ConstitutionalPrinciple>,
        critique_chain: LLMChain<M>,
        revision_chain: LLMChain<M>,
    ) -> Self {
        Self {
            chain: Arc::new(chain),
            constitutional_principles,
            critique_chain: Arc::new(critique_chain),
            revision_chain: Arc::new(revision_chain),
            return_intermediate_steps: false,
        }
    }

    /// Create a `ConstitutionalChain` from an LLM.
    ///
    /// This is a convenience constructor that creates critique and revision chains
    /// using default prompts.
    ///
    /// # Arguments
    ///
    /// * `llm` - The LLM to use for critique and revision
    /// * `chain` - Initial chain to generate responses
    /// * `constitutional_principles` - Principles to apply
    pub fn from_llm(
        llm: Arc<M>,
        chain: LLMChain<M>,
        constitutional_principles: Vec<ConstitutionalPrinciple>,
    ) -> Result<Self> {
        let critique_chain = LLMChain::new(Arc::clone(&llm), critique_prompt()?);
        let revision_chain = LLMChain::new(llm, revision_prompt()?);

        Ok(Self::new(
            chain,
            constitutional_principles,
            critique_chain,
            revision_chain,
        ))
    }

    /// Set whether to return intermediate steps.
    #[must_use]
    pub fn with_return_intermediate_steps(mut self, return_steps: bool) -> Self {
        self.return_intermediate_steps = return_steps;
        self
    }

    /// Get input keys for this chain (delegates to initial chain).
    #[must_use]
    pub fn get_input_keys(&self) -> Vec<String> {
        // Delegate to the initial chain
        vec![] // Would need to access chain.get_input_keys() if available
    }

    /// Get output keys for this chain.
    #[must_use]
    pub fn get_output_keys(&self) -> Vec<String> {
        if self.return_intermediate_steps {
            vec![
                "output".to_string(),
                "initial_output".to_string(),
                "critiques_and_revisions".to_string(),
            ]
        } else {
            vec!["output".to_string()]
        }
    }

    /// Get the chain type identifier.
    #[must_use]
    pub fn chain_type(&self) -> &'static str {
        "constitutional_chain"
    }

    /// Run the constitutional chain.
    ///
    /// # Arguments
    ///
    /// * `inputs` - Input variables for the initial chain
    ///
    /// # Returns
    ///
    /// A `HashMap` containing:
    /// - "output": The final revised response
    /// - "`initial_output"`: The initial response (if `return_intermediate_steps=true`)
    /// - "`critiques_and_revisions"`: Vec of (critique, revision) pairs (if `return_intermediate_steps=true`)
    pub async fn call(&self, inputs: &HashMap<String, String>) -> Result<HashMap<String, String>> {
        // Generate initial response
        let mut response = self.chain.run(inputs).await?;
        let initial_response = response.clone();

        // Format the input prompt (reconstruct from inputs)
        let input_prompt = self
            .chain
            .prompt()
            .format(inputs)
            .map_err(|e| Error::Other(format!("Failed to format prompt: {e}")))?;

        let mut critiques_and_revisions = Vec::new();

        // Apply each constitutional principle
        for principle in &self.constitutional_principles {
            // Generate critique
            let mut critique_inputs = HashMap::new();
            critique_inputs.insert("input_prompt".to_string(), input_prompt.clone());
            critique_inputs.insert("output_from_model".to_string(), response.clone());
            critique_inputs.insert(
                "critique_request".to_string(),
                principle.critique_request.clone(),
            );

            let raw_critique = self.critique_chain.run(&critique_inputs).await?;
            let critique = parse_critique(&raw_critique);

            // If "no critique needed", skip revision
            if critique.to_lowercase().contains("no critique needed") {
                critiques_and_revisions.push((critique, String::new()));
                continue;
            }

            // Generate revision
            let mut revision_inputs = HashMap::new();
            revision_inputs.insert("input_prompt".to_string(), input_prompt.clone());
            revision_inputs.insert("output_from_model".to_string(), response.clone());
            revision_inputs.insert(
                "critique_request".to_string(),
                principle.critique_request.clone(),
            );
            revision_inputs.insert("critique".to_string(), critique.clone());
            revision_inputs.insert(
                "revision_request".to_string(),
                principle.revision_request.clone(),
            );

            let revision = self.revision_chain.run(&revision_inputs).await?;
            let revision = revision.trim().to_string();

            // Update response to the revision
            response = revision.clone();
            critiques_and_revisions.push((critique, revision));
        }

        // Build output
        let mut output = HashMap::new();
        output.insert("output".to_string(), response);

        if self.return_intermediate_steps {
            output.insert("initial_output".to_string(), initial_response);
            // Serialize critiques_and_revisions as JSON
            let critiques_json = serde_json::to_string(&critiques_and_revisions)
                .unwrap_or_else(|_| "[]".to_string());
            output.insert("critiques_and_revisions".to_string(), critiques_json);
        }

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constitutional_principle_creation() {
        let principle = ConstitutionalPrinciple::new("test", "Is this good?", "Make it better.");

        assert_eq!(principle.name, "test");
        assert_eq!(principle.critique_request, "Is this good?");
        assert_eq!(principle.revision_request, "Make it better.");
    }

    #[test]
    fn test_built_in_principles() {
        let harmful1 = ConstitutionalPrinciple::harmful1();
        assert_eq!(harmful1.name, "harmful1");
        assert!(harmful1.critique_request.contains("harmful"));
        assert!(harmful1.revision_request.contains("rewrite"));

        let thoughtful = ConstitutionalPrinciple::thoughtful();
        assert_eq!(thoughtful.name, "thoughtful");
        assert!(thoughtful.critique_request.contains("thoughtful"));
    }

    #[test]
    fn test_all_principles() {
        let all = ConstitutionalPrinciple::all_principles();
        assert!(all.len() >= 11); // At least 11 built-in principles
    }

    #[test]
    fn test_get_principles() {
        let principles = ConstitutionalPrinciple::get_principles(&["harmful1", "thoughtful"]);
        assert_eq!(principles.len(), 2);
        assert_eq!(principles[0].name, "harmful1");
        assert_eq!(principles[1].name, "thoughtful");
    }

    #[test]
    fn test_parse_critique_basic() {
        let output = "This is harmful. Critique Needed.";
        let parsed = parse_critique(output);
        assert_eq!(parsed, "This is harmful. Critique Needed.");
    }

    #[test]
    fn test_parse_critique_with_revision_request() {
        let output = "This is harmful. Critique Needed.\n\nRevision request: Please fix it.";
        let parsed = parse_critique(output);
        assert_eq!(parsed, "This is harmful. Critique Needed.");
    }

    #[test]
    fn test_parse_critique_with_extra_paragraphs() {
        let output = "This is harmful.\n\nMore text here.\n\nEven more.";
        let parsed = parse_critique(output);
        assert_eq!(parsed, "This is harmful.");
    }

    #[test]
    fn test_critique_prompt() {
        let prompt = critique_prompt().unwrap();
        assert!(prompt.template.contains("Critique Request"));
        assert!(prompt.template.contains("input_prompt"));
    }

    #[test]
    fn test_revision_prompt() {
        let prompt = revision_prompt().unwrap();
        assert!(prompt.template.contains("Revision Request"));
        assert!(prompt.template.contains("critique"));
    }
}
