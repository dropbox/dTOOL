// LangSmith tracing with OpenAI chat model
//
// This example demonstrates end-to-end tracing of LLM calls to LangSmith.
//
// Build with: cargo build -p dashflow-openai --example tracing_with_langsmith --features dashflow::core/tracing
// Run with: cargo run -p dashflow-openai --example tracing_with_langsmith --features dashflow::core/tracing
//
// Required environment variables:
// - OPENAI_API_KEY: Your OpenAI API key (required for LLM calls)
// - LANGSMITH_API_KEY: Your LangSmith API key (required for tracing, get from https://smith.dashflow.com)
// - LANGSMITH_PROJECT: Optional project name (defaults to "default")

use dashflow::core::callbacks::CallbackManager;
use dashflow::core::config::RunnableConfig;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow::core::tracers::DashFlowTracer;
use dashflow_openai::ChatOpenAI;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== LangSmith Tracing with OpenAI ===\n");

    // Check for OpenAI API key
    let openai_key = std::env::var("OPENAI_API_KEY").ok();
    if openai_key.is_none() {
        println!("⚠️  OPENAI_API_KEY not set");
        println!("   Set it to enable actual LLM calls\n");
    }

    // Create LangSmith tracer
    println!("Creating DashFlow tracer...");
    match DashFlowTracer::new() {
        Ok(tracer) => {
            println!("✅ DashFlow tracer created successfully");
            println!("   Traces will be sent to LangSmith\n");

            // Create callback manager with the tracer
            let mut callback_manager = CallbackManager::new();
            callback_manager.add_handler(Arc::new(tracer));

            // Create runnable config with callbacks
            let config = RunnableConfig::default().with_callbacks(callback_manager);

            // Only make actual LLM calls if OpenAI key is available
            if openai_key.is_some() {
                println!("Making OpenAI API call with tracing enabled...\n");

                // Create ChatOpenAI instance
                let chat = ChatOpenAI::with_config(Default::default()).with_model("gpt-4o-mini");

                // Create messages
                let messages = vec![
                    Message::system("You are a helpful assistant."),
                    Message::human("What is the capital of France? Answer in one word."),
                ];

                // Make the call with tracing
                match chat
                    .generate(&messages, None, None, None, Some(&config))
                    .await
                {
                    Ok(result) => {
                        println!("✅ LLM call completed successfully");
                        if let Some(gen) = result.generations.first() {
                            println!("   Response: {}\n", gen.message.as_text());
                        }

                        println!("Trace information:");
                        println!("- Run was traced and sent to LangSmith");
                        println!("- Callbacks captured: on_llm_start, on_llm_end");
                        println!("- View traces at: https://smith.dashflow.com\n");

                        // Make a second call to demonstrate multiple traces
                        println!("Making second API call with tracing...\n");
                        let messages2 = vec![
                            Message::system("You are a helpful assistant."),
                            Message::human("What is 2 + 2? Answer with just the number."),
                        ];

                        match chat
                            .generate(&messages2, None, None, None, Some(&config))
                            .await
                        {
                            Ok(result2) => {
                                println!("✅ Second LLM call completed");
                                if let Some(gen) = result2.generations.first() {
                                    println!("   Response: {}\n", gen.message.as_text());
                                }
                                println!("Both traces have been sent to LangSmith\n");
                            }
                            Err(e) => {
                                println!("⚠️  Second call failed: {}\n", e);
                            }
                        }
                    }
                    Err(e) => {
                        println!("⚠️  LLM call failed: {}", e);
                        println!("   Error was traced to LangSmith (on_llm_error)\n");
                    }
                }
            } else {
                println!("LLM calls disabled (no OPENAI_API_KEY)");
                println!("Set OPENAI_API_KEY to enable actual tracing\n");
            }

            println!("Tracing Infrastructure:");
            println!("- BaseTracer trait: Defines persist_run() for custom persistence");
            println!("- DashFlowTracer: Sends traces to LangSmith via batch queue");
            println!("- RunTree: Hierarchical execution trace structure");
            println!("- CallbackHandler: Lifecycle hooks (on_llm_start, on_llm_end, on_llm_error)");
            println!("- ChatModel integration: All 10 providers support tracing\n");

            Ok(())
        }
        Err(e) => {
            println!("⚠️  Could not create tracer: {}", e);
            println!("   This is expected if LANGSMITH_API_KEY is not set\n");

            println!("To use LangSmith tracing:");
            println!("1. Get API key from: https://smith.dashflow.com");
            println!("2. Set environment variable: export LANGSMITH_API_KEY=your_key");
            println!("3. Set OpenAI key: export OPENAI_API_KEY=your_openai_key");
            println!("4. Optionally set project: export LANGSMITH_PROJECT=my_project");
            println!("5. Re-run this example\n");

            Ok(())
        }
    }
}
