//! Question answering with sources over documents.
//!
//! This module provides chains for answering questions with source citations.
//! The `QAWithSourcesChain` extracts sources from the LLM response and returns
//! them separately, making it easy to show users which documents were used.
//!
//! # Examples
//!
//! ## Basic Usage with Documents
//!
//! ```rust,ignore
//! use dashflow_chains::qa_with_sources::{QAWithSourcesChain, qa_with_sources_prompts};
//! use dashflow_chains::combine_documents::StuffDocumentsChain;
//! use dashflow_chains::llm::LLMChain;
//! use dashflow::core::documents::Document;
//!
//! // Create the document combining chain
//! let llm_chain = LLMChain::new(llm, qa_with_sources_prompts::stuff_prompt());
//! let combine_docs_chain = StuffDocumentsChain::new(llm_chain)
//!     .with_document_prompt(qa_with_sources_prompts::document_prompt())
//!     .with_document_variable_name("summaries");
//!
//! // Create the QA with sources chain
//! let chain = QAWithSourcesChain::new(combine_docs_chain);
//!
//! // Use it
//! let docs = vec![
//!     Document::new("Paris is the capital of France.").with_metadata("source", "doc1"),
//!     Document::new("Berlin is the capital of Germany.").with_metadata("source", "doc2"),
//! ];
//!
//! let result = chain.invoke(QAInput {
//!     question: "What is the capital of France?".to_string(),
//!     docs,
//! }).await?;
//!
//! println!("Answer: {}", result.answer);
//! println!("Sources: {}", result.sources);
//! ```
//!
//! ## Using with Retriever
//!
//! ```rust,ignore
//! use dashflow_chains::qa_with_sources::RetrievalQAWithSourcesChain;
//!
//! let chain = RetrievalQAWithSourcesChain::builder()
//!     .combine_documents_chain(combine_docs_chain)
//!     .retriever(my_retriever)
//!     .build()?;
//!
//! let result = chain.invoke("What is the capital of France?").await?;
//! ```

use dashflow::core::documents::Document;
use dashflow::core::retrievers::Retriever;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::combine_documents::StuffDocumentsChain;

/// Input for `QAWithSourcesChain`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QAInput {
    /// The question to answer
    pub question: String,
    /// Documents to use for answering
    pub docs: Vec<Document>,
}

/// Input for `RetrievalQAWithSourcesChain`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalQAInput {
    /// The question to answer
    pub question: String,
}

/// Output from QA with sources chains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QAWithSourcesOutput {
    /// The answer to the question
    pub answer: String,
    /// Source citations (comma-separated)
    pub sources: String,
    /// Optional: the source documents (if `return_source_documents` is true)
    pub source_documents: Option<Vec<Document>>,
}

/// Question answering with sources over documents.
///
/// This chain takes documents and a question, processes them through a document
/// combining chain, and extracts the answer and sources from the LLM output.
///
/// The LLM is expected to return sources in the format:
/// ```text
/// FINAL ANSWER: The answer text here.
/// SOURCES: source1, source2
/// ```
pub struct QAWithSourcesChain {
    /// Chain to use to combine documents
    combine_documents_chain: Arc<StuffDocumentsChain>,
    /// Key for the question in inputs (default: "question")
    question_key: String,
    /// Key for documents in inputs (default: "docs")
    input_docs_key: String,
    /// Whether to return the source documents in output
    return_source_documents: bool,
}

impl QAWithSourcesChain {
    /// Create a new `QAWithSourcesChain`
    #[must_use]
    pub fn new(combine_documents_chain: StuffDocumentsChain) -> Self {
        Self {
            combine_documents_chain: Arc::new(combine_documents_chain),
            question_key: "question".to_string(),
            input_docs_key: "docs".to_string(),
            return_source_documents: false,
        }
    }

    /// Set the question key
    pub fn with_question_key(mut self, key: impl Into<String>) -> Self {
        self.question_key = key.into();
        self
    }

    /// Set the input docs key
    pub fn with_input_docs_key(mut self, key: impl Into<String>) -> Self {
        self.input_docs_key = key.into();
        self
    }

    /// Set whether to return source documents
    #[must_use]
    pub fn with_return_source_documents(mut self, return_docs: bool) -> Self {
        self.return_source_documents = return_docs;
        self
    }

    /// Split sources from the answer text.
    ///
    /// Looks for "SOURCES:" (case-insensitive) in the answer and extracts
    /// the sources that follow.
    fn split_sources(&self, answer: &str) -> (String, String) {
        // Match Python regex: r"SOURCES?:" case-insensitive
        // SAFETY: M-347 - Compile-time constant regex pattern
        #[allow(clippy::expect_used)]
        let re = Regex::new(r"(?i)SOURCES?:|QUESTION:\s").expect("sources regex is valid");

        if re.is_match(answer) {
            let parts: Vec<&str> = re.splitn(answer, 3).collect();
            if parts.len() >= 2 {
                let answer_part = parts[0].trim();
                let sources_part = parts[1].lines().next().unwrap_or("").trim().to_string();
                return (answer_part.to_string(), sources_part);
            }
        }

        (answer.to_string(), String::new())
    }

    /// Process the question and documents
    pub async fn invoke(
        &self,
        question: impl Into<String>,
        docs: Vec<Document>,
    ) -> Result<QAWithSourcesOutput, Box<dyn std::error::Error>> {
        let question = question.into();

        // Build inputs for the combine documents chain
        let mut inputs = HashMap::new();
        inputs.insert("question".to_string(), question);

        // Combine documents
        let (answer, _) = self
            .combine_documents_chain
            .combine_docs(&docs, Some(inputs))
            .await?;

        // Split sources from answer
        let (answer, sources) = self.split_sources(&answer);

        Ok(QAWithSourcesOutput {
            answer,
            sources,
            source_documents: if self.return_source_documents {
                Some(docs)
            } else {
                None
            },
        })
    }

    /// Process with structured input
    pub async fn invoke_with_input(
        &self,
        input: QAInput,
    ) -> Result<QAWithSourcesOutput, Box<dyn std::error::Error>> {
        self.invoke(input.question, input.docs).await
    }
}

/// Question answering with sources that retrieves documents using a retriever.
///
/// This chain uses a retriever to fetch relevant documents for the question,
/// then processes them through the QA with sources chain.
pub struct RetrievalQAWithSourcesChain {
    /// The underlying QA with sources chain
    qa_chain: QAWithSourcesChain,
    /// Retriever to fetch documents
    retriever: Arc<dyn Retriever>,
    /// Reduce retrieved docs if they exceed token limit
    reduce_k_below_max_tokens: bool,
    /// Maximum token limit for documents (only used if `reduce_k_below_max_tokens` is true)
    max_tokens_limit: usize,
}

impl RetrievalQAWithSourcesChain {
    /// Create a builder for `RetrievalQAWithSourcesChain`
    #[must_use]
    pub fn builder() -> RetrievalQAWithSourcesChainBuilder {
        RetrievalQAWithSourcesChainBuilder::default()
    }

    /// Reduce documents below token limit if necessary
    fn reduce_tokens_below_limit(&self, docs: Vec<Document>) -> Vec<Document> {
        if !self.reduce_k_below_max_tokens {
            return docs;
        }

        // For StuffDocumentsChain, we can access the LLM chain
        // This is a simplified implementation - in practice, you'd need
        // to get token count from the LLM chain
        let mut token_count = 0;
        let mut num_docs = 0;

        for doc in &docs {
            // Rough token estimate: 1 token per 4 characters
            let tokens = doc.page_content.len() / 4;
            if token_count + tokens > self.max_tokens_limit {
                break;
            }
            token_count += tokens;
            num_docs += 1;
        }

        docs.into_iter().take(num_docs).collect()
    }

    /// Invoke with a question string
    pub async fn invoke(
        &self,
        question: impl Into<String>,
    ) -> Result<QAWithSourcesOutput, Box<dyn std::error::Error>> {
        let question = question.into();

        // Retrieve documents
        let docs = self
            .retriever
            ._get_relevant_documents(&question, None)
            .await?;

        // Reduce if necessary
        let docs = self.reduce_tokens_below_limit(docs);

        // Process through QA chain
        self.qa_chain.invoke(question, docs).await
    }

    /// Invoke with structured input
    pub async fn invoke_with_input(
        &self,
        input: RetrievalQAInput,
    ) -> Result<QAWithSourcesOutput, Box<dyn std::error::Error>> {
        self.invoke(input.question).await
    }
}

/// Builder for `RetrievalQAWithSourcesChain`
#[derive(Default)]
pub struct RetrievalQAWithSourcesChainBuilder {
    combine_documents_chain: Option<Arc<StuffDocumentsChain>>,
    retriever: Option<Arc<dyn Retriever>>,
    reduce_k_below_max_tokens: bool,
    max_tokens_limit: usize,
    return_source_documents: bool,
}

impl RetrievalQAWithSourcesChainBuilder {
    /// Set the document combining chain
    #[must_use]
    pub fn combine_documents_chain(mut self, chain: StuffDocumentsChain) -> Self {
        self.combine_documents_chain = Some(Arc::new(chain));
        self
    }

    /// Set the retriever
    pub fn retriever(mut self, retriever: impl Retriever + 'static) -> Self {
        self.retriever = Some(Arc::new(retriever));
        self
    }

    /// Set whether to reduce documents below token limit
    #[must_use]
    pub fn reduce_k_below_max_tokens(mut self, reduce: bool) -> Self {
        self.reduce_k_below_max_tokens = reduce;
        self
    }

    /// Set the maximum token limit
    #[must_use]
    pub fn max_tokens_limit(mut self, limit: usize) -> Self {
        self.max_tokens_limit = limit;
        self
    }

    /// Set whether to return source documents
    #[must_use]
    pub fn return_source_documents(mut self, return_docs: bool) -> Self {
        self.return_source_documents = return_docs;
        self
    }

    /// Build the chain
    pub fn build(self) -> Result<RetrievalQAWithSourcesChain, &'static str> {
        let combine_documents_chain = self
            .combine_documents_chain
            .ok_or("combine_documents_chain is required")?;
        let retriever = self.retriever.ok_or("retriever is required")?;

        let qa_chain = QAWithSourcesChain {
            combine_documents_chain,
            question_key: "question".to_string(),
            input_docs_key: "docs".to_string(),
            return_source_documents: self.return_source_documents,
        };

        Ok(RetrievalQAWithSourcesChain {
            qa_chain,
            retriever,
            reduce_k_below_max_tokens: self.reduce_k_below_max_tokens,
            max_tokens_limit: if self.max_tokens_limit > 0 {
                self.max_tokens_limit
            } else {
                3375 // Default from Python implementation
            },
        })
    }
}

/// Prompts for QA with sources chains
pub mod qa_with_sources_prompts {
    use dashflow::core::prompts::PromptTemplate;

    /// Get the default prompt for the "stuff" chain type
    #[must_use]
    pub fn stuff_prompt() -> PromptTemplate {
        #[allow(clippy::expect_used)]
        PromptTemplate::from_template(
            r#"Given the following extracted parts of a long document and a question, create a final answer with references ("SOURCES").
If you don't know the answer, just say that you don't know. Don't try to make up an answer.
ALWAYS return a "SOURCES" part in your answer.

QUESTION: Which state/country's law governs the interpretation of the contract?
=========
Content: This Agreement is governed by English law and the parties submit to the exclusive jurisdiction of the English courts in  relation to any dispute (contractual or non-contractual) concerning this Agreement save that either party may apply to any court for an  injunction or other relief to protect its Intellectual Property Rights.
Source: 28-pl
Content: No Waiver. Failure or delay in exercising any right or remedy under this Agreement shall not constitute a waiver of such (or any other)  right or remedy.

11.7 Severability. The invalidity, illegality or unenforceability of any term (or part of a term) of this Agreement shall not affect the continuation  in force of the remainder of the term (if any) and this Agreement.

11.8 No Agency. Except as expressly stated otherwise, nothing in this Agreement shall create an agency, partnership or joint venture of any  kind between the parties.

11.9 No Third-Party Beneficiaries.
Source: 30-pl
Content: (b) if Google believes, in good faith, that the Distributor has violated or caused Google to violate any Anti-Bribery Laws (as  defined in Clause 8.5) or that such a violation is reasonably likely to occur,
Source: 4-pl
=========
FINAL ANSWER: This Agreement is governed by English law.
SOURCES: 28-pl

QUESTION: What did the president say about Michael Jackson?
=========
Content: Madam Speaker, Madam Vice President, our First Lady and Second Gentleman. Members of Congress and the Cabinet. Justices of the Supreme Court. My fellow Americans.

Last year COVID-19 kept us apart. This year we are finally together again.

Tonight, we meet as Democrats Republicans and Independents. But most importantly as Americans.

With a duty to one another to the American people to the Constitution.

And with an unwavering resolve that freedom will always triumph over tyranny.

Six days ago, Russia's Vladimir Putin sought to shake the foundations of the free world thinking he could make it bend to his menacing ways. But he badly miscalculated.

He thought he could roll into Ukraine and the world would roll over. Instead he met a wall of strength he never imagined.

He met the Ukrainian people.

From President Zelenskyy to every Ukrainian, their fearlessness, their courage, their determination, inspires the world.

Groups of citizens blocking tanks with their bodies. Everyone from students to retirees teachers turned soldiers defending their homeland.
Source: 0-pl
Content: And we won't stop.

We have lost so much to COVID-19. Time with one another. And worst of all, so much loss of life.

Let's use this moment to reset. Let's stop looking at COVID-19 as a partisan dividing line and see it for what it is: A God-awful disease.

Let's stop seeing each other as enemies, and start seeing each other for who we really are: Fellow Americans.

We can't change how divided we've been. But we can change how we move forward—on COVID-19 and other issues we must face together.

I recently visited the New York City Police Department days after the funerals of Officer Wilbert Mora and his partner, Officer Jason Rivera.

They were responding to a 9-1-1 call when a man shot and killed them with a stolen gun.

Officer Mora was 27 years old.

Officer Rivera was 22.

Both Dominican Americans who'd grown up on the same streets they later chose to patrol as police officers.

I spoke with their families and told them that we are forever in debt for their sacrifice, and we will carry on their mission to restore the trust and safety every community deserves.
Source: 24-pl
Content: And a proud Ukrainian people, who have known 30 years  of independence, have repeatedly shown that they will not tolerate anyone who tries to take their country backwards.

To all Americans, I will be honest with you, as I've always promised. A Russian dictator, invading a foreign country, has costs around the world.

And I'm taking robust action to make sure the pain of our sanctions  is targeted at Russia's economy. And I will use every tool at our disposal to protect American businesses and consumers.

Tonight, I can announce that the United States has worked with 30 other countries to release 60 Million barrels of oil from reserves around the world.

America will lead that effort, releasing 30 Million barrels from our own Strategic Petroleum Reserve. And we stand ready to do more if necessary, unified with our allies.

These steps will help blunt gas prices here at home. And I know the news about what's happening can seem alarming.

But I want you to know that we are going to be okay.
Source: 5-pl
Content: More support for patients and families.

To get there, I call on Congress to fund ARPA-H, the Advanced Research Projects Agency for Health.

It's based on DARPA—the Defense Department project that led to the Internet, GPS, and so much more.

ARPA-H will have a singular purpose—to drive breakthroughs in cancer, Alzheimer's, diabetes, and more.

A unity agenda for the nation.

We can do this.

My fellow Americans—tonight , we have gathered in a sacred space—the citadel of our democracy.

In this Capitol, generation after generation, Americans have debated great questions amid great strife, and have done great things.

We have fought for freedom, expanded liberty, defeated totalitarianism and terror.

And built the strongest, freest, and most prosperous nation the world has ever known.

Now is the hour.

Our moment of responsibility.

Our test of resolve and conscience, of history itself.

It is in this moment that our character is formed. Our purpose is found. Our future is forged.

Well I know this nation.
Source: 34-pl
=========
FINAL ANSWER: The president did not mention Michael Jackson.
SOURCES:

QUESTION: {question}
=========
{summaries}
=========
FINAL ANSWER:"#,
        )
        .expect("Valid prompt template")
    }

    /// Get the document prompt for formatting individual documents
    #[must_use]
    pub fn document_prompt() -> PromptTemplate {
        #[allow(clippy::expect_used)]
        PromptTemplate::from_template("Content: {page_content}\nSource: {source}")
            .expect("Valid prompt template")
    }

    /// Get the question prompt for map-reduce (map step)
    #[must_use]
    pub fn map_reduce_question_prompt() -> PromptTemplate {
        #[allow(clippy::expect_used)]
        PromptTemplate::from_template(
            r"Use the following portion of a long document to see if any of the text is relevant to answer the question.
Return any relevant text verbatim.
{context}
Question: {question}
Relevant text, if any:",
        )
        .expect("Valid prompt template")
    }

    /// Get the combine prompt for map-reduce (reduce step)
    #[must_use]
    pub fn map_reduce_combine_prompt() -> PromptTemplate {
        // Same as stuff_prompt
        stuff_prompt()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_sources() {
        use dashflow::core::prompts::PromptTemplate;

        // Create a minimal chain just for testing split_sources
        let stuff_chain = StuffDocumentsChain::new_llm(Arc::new(
            dashflow::core::language_models::FakeLLM::new(vec!["test response".to_string()]),
        ))
        .with_prompt(PromptTemplate::from_template("test {context}").unwrap());

        let chain = QAWithSourcesChain::new(stuff_chain);

        // Test with sources
        let (answer, sources) = chain.split_sources("The answer is Paris.\nSOURCES: doc1, doc2");
        assert_eq!(answer, "The answer is Paris.");
        assert_eq!(sources, "doc1, doc2");

        // Test with SOURCE (singular)
        let (answer, sources) = chain.split_sources("The answer is Berlin.\nSOURCE: doc3");
        assert_eq!(answer, "The answer is Berlin.");
        assert_eq!(sources, "doc3");

        // Test case insensitivity
        let (answer, sources) = chain.split_sources("Answer text.\nsources: doc4");
        assert_eq!(answer, "Answer text.");
        assert_eq!(sources, "doc4");

        // Test without sources
        let (answer, sources) = chain.split_sources("Just an answer.");
        assert_eq!(answer, "Just an answer.");
        assert_eq!(sources, "");
    }
}
