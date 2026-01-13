// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @cli dashflow train finetune
//! @cli-status wired
//!
//! OpenAI fine-tuning student implementation.
//!
//! Uses OpenAI's fine-tuning API to create a specialized version of gpt-3.5-turbo
//! trained on the teacher's examples.

use crate::constants::{
    DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT, DEFAULT_POOL_IDLE_TIMEOUT,
    DEFAULT_TCP_KEEPALIVE,
};
use crate::core::config_loader::env_vars::{
    openai_api_url, DEFAULT_OPENAI_FILES_ENDPOINT, DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT,
};
use crate::{Error, GraphState, Result};
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::time::{Duration, Instant};

/// Student that learns via OpenAI's fine-tuning API.
pub struct OpenAIFineTuneStudent<S: GraphState> {
    api_key: String,
    base_model: String,
    client: reqwest::Client,
    _phantom: PhantomData<S>,
}

#[derive(Serialize, Deserialize, Debug)]
struct FineTuneMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct FineTuneExample {
    messages: Vec<FineTuneMessage>,
}

#[derive(Deserialize, Debug)]
struct UploadResponse {
    id: String,
}

#[derive(Deserialize, Debug)]
struct FineTuneJobResponse {
    id: String,
    status: String,
    fine_tuned_model: Option<String>,
}

impl<S: GraphState> OpenAIFineTuneStudent<S> {
    /// Creates a new OpenAI fine-tuning student.
    ///
    /// # Arguments
    ///
    /// * `api_key` - OpenAI API key
    /// * `base_model` - Base model to fine-tune (default: "gpt-3.5-turbo")
    pub fn new(api_key: String, base_model: Option<String>) -> Self {
        // Build HTTP client using centralized timeout constants
        let builder = reqwest::Client::builder()
            .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
            .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
            .pool_max_idle_per_host(32)
            .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT)
            .tcp_keepalive(DEFAULT_TCP_KEEPALIVE);
        let builder = crate::core::http_client::apply_platform_proxy_config(builder);
        let client = builder.build().unwrap_or_else(|_| {
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("Failed to build HTTP client")
        });

        Self {
            api_key,
            base_model: base_model.unwrap_or_else(|| "gpt-3.5-turbo".to_string()),
            client,
            _phantom: PhantomData,
        }
    }

    /// Returns the base model being fine-tuned.
    pub fn base_model(&self) -> &str {
        &self.base_model
    }

    /// Fine-tunes a model using the provided training data.
    ///
    /// This process:
    /// 1. Formats training data for OpenAI
    /// 2. Uploads training file
    /// 3. Creates fine-tuning job
    /// 4. Polls until complete
    /// 5. Returns fine-tuned model ID
    ///
    /// # Arguments
    ///
    /// * `training_data` - Labeled examples from teacher
    ///
    /// # Returns
    ///
    /// The fine-tuned model ID (e.g., "ft:gpt-3.5-turbo:org:model:id")
    pub async fn fine_tune(&self, training_data: Vec<S>) -> Result<String> {
        // 1. Format training data
        let formatted = self.format_for_openai(&training_data)?;

        // 2. Upload training file
        let file_id = self.upload_training_file(&formatted).await?;

        // 3. Create fine-tuning job
        let job_id = self.create_fine_tune_job(&file_id).await?;

        // 4. Poll until complete
        let model_id = self.wait_for_completion(&job_id).await?;

        Ok(model_id)
    }

    /// Formats examples into OpenAI's fine-tuning format (JSONL with messages).
    ///
    /// Extracts question/answer from GraphState by serializing to JSON and looking
    /// for common field names: "question", "input", "query" for user content and
    /// "answer", "output", "response" for assistant content.
    fn format_for_openai(&self, examples: &[S]) -> Result<String> {
        let mut lines = Vec::new();

        for example in examples {
            // Serialize state to JSON to extract fields
            let json_value = serde_json::to_value(example)
                .map_err(|e| Error::Validation(format!("Failed to serialize state: {}", e)))?;

            // Extract user content from common input field names
            let user_content =
                Self::extract_field(&json_value, &["question", "input", "query", "prompt"])
                    .unwrap_or_else(|| {
                        // Fallback: use full JSON representation
                        json_value.to_string()
                    });

            // Extract assistant content from common output field names
            let assistant_content =
                Self::extract_field(&json_value, &["answer", "output", "response", "result"])
                    .unwrap_or_else(|| {
                        // Fallback: empty string if no output field found
                        String::new()
                    });

            let ft_example = FineTuneExample {
                messages: vec![
                    FineTuneMessage {
                        role: "user".to_string(),
                        content: user_content,
                    },
                    FineTuneMessage {
                        role: "assistant".to_string(),
                        content: assistant_content,
                    },
                ],
            };

            lines.push(serde_json::to_string(&ft_example).map_err(|e| {
                Error::Validation(format!("Failed to serialize training example: {}", e))
            })?);
        }

        Ok(lines.join("\n"))
    }

    /// Extracts a string value from a JSON object by trying multiple field names.
    fn extract_field(value: &serde_json::Value, field_names: &[&str]) -> Option<String> {
        if let serde_json::Value::Object(map) = value {
            for field_name in field_names {
                if let Some(field_value) = map.get(*field_name) {
                    return match field_value {
                        serde_json::Value::String(s) => Some(s.clone()),
                        other => Some(other.to_string()),
                    };
                }
            }
        }
        None
    }

    /// Uploads training file to OpenAI.
    async fn upload_training_file(&self, content: &str) -> Result<String> {
        tracing::info!("Uploading training file ({} bytes)", content.len());

        // Create multipart form with the JSONL content
        let form = reqwest::multipart::Form::new()
            .text("purpose", "fine-tune")
            .part(
                "file",
                reqwest::multipart::Part::text(content.to_string())
                    .file_name("training_data.jsonl")
                    .mime_str("application/json")
                    .map_err(|e| Error::Validation(format!("Invalid MIME type: {}", e)))?,
            );

        // Upload to OpenAI Files API
        let response = self
            .client
            .post(openai_api_url(DEFAULT_OPENAI_FILES_ENDPOINT))
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| Error::Validation(format!("Failed to upload training file: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            return Err(Error::Validation(format!(
                "Upload failed with status {}: {}",
                status, error_text
            )));
        }

        let upload_response: UploadResponse = response
            .json()
            .await
            .map_err(|e| Error::Validation(format!("Failed to parse upload response: {}", e)))?;

        tracing::info!("File uploaded successfully: {}", upload_response.id);
        Ok(upload_response.id)
    }

    /// Creates a fine-tuning job.
    async fn create_fine_tune_job(&self, file_id: &str) -> Result<String> {
        tracing::info!("Creating fine-tune job for file {}", file_id);

        #[derive(Serialize)]
        struct CreateJobRequest {
            model: String,
            training_file: String,
        }

        let request = CreateJobRequest {
            model: self.base_model.clone(),
            training_file: file_id.to_string(),
        };

        let response = self
            .client
            .post(openai_api_url(DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT))
            .bearer_auth(&self.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Validation(format!("Failed to create fine-tuning job: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read error response".to_string());
            return Err(Error::Validation(format!(
                "Job creation failed with status {}: {}",
                status, error_text
            )));
        }

        let job_response: FineTuneJobResponse = response.json().await.map_err(|e| {
            Error::Validation(format!("Failed to parse job creation response: {}", e))
        })?;

        tracing::info!("Fine-tuning job created: {}", job_response.id);
        Ok(job_response.id)
    }

    /// Maximum wait time for fine-tuning jobs (24 hours).
    /// OpenAI fine-tuning can take several hours for large datasets.
    const MAX_WAIT_DURATION: Duration = Duration::from_secs(24 * 60 * 60);

    /// Waits for fine-tuning job to complete.
    async fn wait_for_completion(&self, job_id: &str) -> Result<String> {
        tracing::info!("Waiting for job {} to complete", job_id);

        let poll_interval = Duration::from_secs(20); // Poll every 20 seconds
        let mut last_event_id: Option<String> = None;
        let start = Instant::now();

        loop {
            // Check timeout
            if start.elapsed() > Self::MAX_WAIT_DURATION {
                return Err(Error::Validation(format!(
                    "Fine-tuning job {} did not complete within {:?}",
                    job_id,
                    Self::MAX_WAIT_DURATION
                )));
            }

            // Get job status
            let response = self
                .client
                .get(format!(
                    "{}/{}",
                    openai_api_url(DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT),
                    job_id
                ))
                .bearer_auth(&self.api_key)
                .send()
                .await
                .map_err(|e| Error::Validation(format!("Failed to get job status: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unable to read error response".to_string());
                return Err(Error::Validation(format!(
                    "Failed to get job status {}: {}",
                    status, error_text
                )));
            }

            let job: FineTuneJobResponse = response.json().await.map_err(|e| {
                Error::Validation(format!("Failed to parse job status response: {}", e))
            })?;

            tracing::debug!("Job {} status: {}", job_id, job.status);

            // Check for terminal statuses
            match job.status.as_str() {
                "succeeded" => {
                    let model_id = job.fine_tuned_model.ok_or_else(|| {
                        Error::Validation(
                            "Job succeeded but no fine-tuned model ID found".to_string(),
                        )
                    })?;
                    tracing::info!("Fine-tuning completed successfully: {}", model_id);
                    return Ok(model_id);
                }
                "failed" => {
                    return Err(Error::Validation(format!(
                        "Fine-tuning job {} failed",
                        job_id
                    )));
                }
                "cancelled" => {
                    return Err(Error::Validation(format!(
                        "Fine-tuning job {} was cancelled",
                        job_id
                    )));
                }
                _ => {
                    // Job still in progress (validating_files, queued, running)
                    // Fetch latest event for progress reporting
                    if let Ok(event_response) = self
                        .client
                        .get(format!(
                            "{}/{}/events?limit=1",
                            openai_api_url(DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT),
                            job_id
                        ))
                        .bearer_auth(&self.api_key)
                        .send()
                        .await
                    {
                        #[derive(Deserialize)]
                        struct EventsResponse {
                            data: Vec<Event>,
                        }

                        #[derive(Deserialize)]
                        struct Event {
                            id: String,
                            message: String,
                        }

                        if let Ok(events) = event_response.json::<EventsResponse>().await {
                            if let Some(event) = events.data.first() {
                                // Only log if it's a new event
                                if last_event_id.as_ref() != Some(&event.id) {
                                    tracing::info!("Progress: {}", event.message);
                                    last_event_id = Some(event.id.clone());
                                }
                            }
                        }
                    }

                    // Wait before next poll
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MergeableState;
    use serde::{Deserialize, Serialize};

    /// Simple test state with just a value field (for basic tests)
    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct TestState {
        value: String,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = other.value.clone();
        }
    }

    /// Realistic test state with question/answer fields
    #[derive(Clone, Debug, Serialize, Deserialize)]
    struct QAState {
        question: String,
        answer: String,
    }

    impl MergeableState for QAState {
        fn merge(&mut self, other: &Self) {
            self.question = other.question.clone();
            self.answer = other.answer.clone();
        }
    }

    #[test]
    fn test_student_creation() {
        let student: OpenAIFineTuneStudent<TestState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), Some("gpt-3.5-turbo".to_string()));
        assert_eq!(student.base_model, "gpt-3.5-turbo");
    }

    #[test]
    fn test_student_default_model() {
        let student: OpenAIFineTuneStudent<TestState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), None);
        assert_eq!(student.base_model(), "gpt-3.5-turbo");
    }

    #[test]
    fn test_student_custom_model() {
        let student: OpenAIFineTuneStudent<TestState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), Some("gpt-4o-mini".to_string()));
        assert_eq!(student.base_model(), "gpt-4o-mini");
    }

    #[test]
    fn test_format_for_openai_creates_jsonl() {
        let student: OpenAIFineTuneStudent<TestState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), None);

        let examples = vec![
            TestState {
                value: "test1".to_string(),
            },
            TestState {
                value: "test2".to_string(),
            },
        ];

        let formatted = student.format_for_openai(&examples).unwrap();

        // Should have 2 lines (one per example)
        let lines: Vec<&str> = formatted.lines().collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.get("messages").is_some());
        }
    }

    #[test]
    fn test_format_for_openai_message_structure() {
        let student: OpenAIFineTuneStudent<TestState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), None);

        let examples = vec![TestState {
            value: "test".to_string(),
        }];

        let formatted = student.format_for_openai(&examples).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();

        let messages = parsed.get("messages").unwrap().as_array().unwrap();
        assert_eq!(messages.len(), 2); // user + assistant

        assert_eq!(messages[0].get("role").unwrap(), "user");
        assert_eq!(messages[1].get("role").unwrap(), "assistant");
    }

    #[test]
    fn test_format_for_openai_empty_examples() {
        let student: OpenAIFineTuneStudent<TestState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), None);

        let examples: Vec<TestState> = vec![];
        let formatted = student.format_for_openai(&examples).unwrap();

        // Should be empty string (no examples)
        assert!(formatted.is_empty());
    }

    #[test]
    fn test_format_for_openai_extracts_question_answer() {
        let student: OpenAIFineTuneStudent<QAState> =
            OpenAIFineTuneStudent::new("test-key".to_string(), None);

        let examples = vec![QAState {
            question: "What is 2+2?".to_string(),
            answer: "4".to_string(),
        }];

        let formatted = student.format_for_openai(&examples).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&formatted).unwrap();

        let messages = parsed.get("messages").unwrap().as_array().unwrap();

        // User message should contain the question
        assert_eq!(messages[0].get("content").unwrap(), "What is 2+2?");
        // Assistant message should contain the answer
        assert_eq!(messages[1].get("content").unwrap(), "4");
    }

    #[test]
    fn test_extract_field_finds_question() {
        let json = serde_json::json!({"question": "test question", "other": "ignored"});
        let result =
            OpenAIFineTuneStudent::<TestState>::extract_field(&json, &["question", "input"]);
        assert_eq!(result, Some("test question".to_string()));
    }

    #[test]
    fn test_extract_field_fallback_to_second_option() {
        let json = serde_json::json!({"input": "test input", "other": "ignored"});
        let result =
            OpenAIFineTuneStudent::<TestState>::extract_field(&json, &["question", "input"]);
        assert_eq!(result, Some("test input".to_string()));
    }

    #[test]
    fn test_extract_field_returns_none_when_not_found() {
        let json = serde_json::json!({"other": "ignored"});
        let result =
            OpenAIFineTuneStudent::<TestState>::extract_field(&json, &["question", "input"]);
        assert_eq!(result, None);
    }
}
