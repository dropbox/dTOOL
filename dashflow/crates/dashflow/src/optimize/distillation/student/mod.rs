// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Student implementations for learning from teacher-generated data.
//!
//! Three approaches to distillation:
//!
//! 1. **OpenAI Fine-tuning**: Fine-tune gpt-3.5-turbo via OpenAI API
//! 2. **Local Fine-tuning**: Fine-tune local models (Llama-3) using MLX + Ollama
//! 3. **Prompt Optimization**: Use BootstrapFewShot for few-shot prompting

pub mod local_finetune;
pub mod openai_finetune;
pub mod prompt_optimization;

pub use local_finetune::LocalFineTuneStudent;
pub use openai_finetune::OpenAIFineTuneStudent;
pub use prompt_optimization::PromptOptimizationStudent;
