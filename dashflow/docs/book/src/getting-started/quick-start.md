# Quick Start

**Last Updated:** 2025-12-19 (Worker #1164 - Fix stale version references)

This guide will walk you through building your first DashFlow application in 10 minutes.

## Your First LLM Call

Let's start with the simplest possible example - calling an LLM.

### Create a New Project

```bash
cargo new my-dashflow-app
cd my-dashflow-app
```

### Add Dependencies

Edit `Cargo.toml`:

```toml
[dependencies]
dashflow = "1.11"
dashflow-openai = "1.11"
tokio = { version = "1", features = ["full"] }
```

### Write Your First Program

Edit `src/main.rs`:

```rust
use dashflow_openai::ChatOpenAI;
use dashflow::core::language_models::ChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the LLM with default settings
    let llm = ChatOpenAI::default()
        .with_model("gpt-4");

    // Simple question
    let response = llm.invoke("What is Rust?").await?;

    println!("{}", response.content);

    Ok(())
}
```

### Run It

```bash
export OPENAI_API_KEY="sk-..."
cargo run
```

**Output:**
```
Rust is a systems programming language that focuses on safety,
speed, and concurrency...
```

## Example 2: Streaming Responses

Stream tokens as they're generated:

```rust
use dashflow_openai::ChatOpenAI;
use dashflow::core::language_models::ChatModel;
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();

    // Stream the response
    let mut stream = llm.stream("Write a haiku about Rust").await?;

    print!("Response: ");
    while let Some(chunk) = stream.next().await {
        print!("{}", chunk?.content);
    }
    println!();

    Ok(())
}
```

**Output** (token by token):
```
Response: Memory safe and fast,
Systems built with zero cost,
Rust empowers all.
```

## Example 3: Prompt Templates

Use templates for consistent prompts:

```rust
use dashflow::core::prompt::PromptTemplate;
use dashflow_openai::ChatOpenAI;
use dashflow::core::language_models::ChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();

    // Create a prompt template
    let template = PromptTemplate::new(
        "You are a helpful assistant. Answer the question:\n\nQuestion: {question}\nAnswer:"
    );

    // Format the prompt
    let prompt = template.format(&[
        ("question", "What is the capital of France?")
    ])?;

    let response = llm.invoke(&prompt).await?;
    println!("{}", response.content);

    Ok(())
}
```

## Example 4: RAG (Retrieval-Augmented Generation)

Build a simple RAG pipeline:

```rust
use dashflow::core::{
    language_models::ChatModel,
    embeddings::Embeddings,
    vector_stores::VectorStore,
    document_loaders::Loader,
    text_splitters::TextSplitter,
};
use dashflow_openai::{ChatOpenAI, OpenAIEmbeddings};
use dashflow_qdrant::QdrantVectorStore;
use dashflow_text_splitters::RecursiveCharacterTextSplitter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load documents
    let loader = dashflow::core::document_loaders::TextLoader::new();
    let documents = loader.load("knowledge_base.txt").await?;

    // 2. Split documents
    let splitter = RecursiveCharacterTextSplitter::new()
        .with_chunk_size(1000)
        .with_chunk_overlap(200);
    let chunks = splitter.split_documents(&documents)?;

    // 3. Create embeddings
    let embeddings = OpenAIEmbeddings::default();

    // 4. Store in vector database
    let vector_store = QdrantVectorStore::new("http://localhost:6333")
        .with_collection("knowledge_base")
        .with_embeddings(embeddings);

    vector_store.add_documents(&chunks).await?;

    // 5. Query the knowledge base
    let query = "What is the main topic?";
    let relevant_docs = vector_store.similarity_search(query, 3).await?;

    // 6. Generate answer with context
    let llm = ChatOpenAI::default();
    let context = relevant_docs.iter()
        .map(|doc| doc.page_content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let prompt = format!(
        "Context:\n{}\n\nQuestion: {}\n\nAnswer:",
        context, query
    );

    let response = llm.invoke(&prompt).await?;
    println!("Answer: {}", response.content);

    Ok(())
}
```

## Example 5: Agent with Tools

Create an agent that can use tools:

```rust
use dashflow::core::{
    language_models::ChatModel,
    agents::{Agent, AgentExecutor},
    tools::Tool,
};
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create LLM
    let llm = ChatOpenAI::default()
        .with_model("gpt-4");

    // Define tools
    let calculator = Tool::new(
        "calculator",
        "Evaluates mathematical expressions",
        |input: &str| async move {
            // Simple calculator implementation
            Ok(format!("Result: {}", eval(input)?))
        }
    );

    let search = Tool::new(
        "search",
        "Searches the internet",
        |query: &str| async move {
            // Mock search implementation
            Ok(format!("Search results for: {}", query))
        }
    );

    // Create agent
    let agent = Agent::new(llm)
        .with_tools(vec![calculator, search]);

    let executor = AgentExecutor::new(agent)
        .with_max_iterations(5);

    // Run agent
    let result = executor.run(
        "What is 25 * 4, and then search for that number"
    ).await?;

    println!("Result: {}", result);

    Ok(())
}
```

## Example 6: Chatbot with Memory

Build a chatbot that remembers conversation history:

```rust
use dashflow::core::{
    language_models::ChatModel,
    memory::{ConversationBufferMemory, Memory},
};
use dashflow_openai::ChatOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let llm = ChatOpenAI::default();
    let mut memory = ConversationBufferMemory::new();

    // Conversation loop
    loop {
        // Get user input
        let mut input = String::new();
        println!("You: ");
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.is_empty() || input == "exit" {
            break;
        }

        // Load conversation history
        let history = memory.load_memory_variables().await?;

        // Create prompt with history
        let prompt = format!(
            "{}\n\nHuman: {}\nAssistant:",
            history.get("history").unwrap_or(&String::new()),
            input
        );

        // Get response
        let response = llm.invoke(&prompt).await?;
        println!("Assistant: {}", response.content);

        // Save to memory
        memory.save_context(input, &response.content).await?;
    }

    Ok(())
}
```

## Common Patterns

### Error Handling

```rust
use dashflow::core::error::DashFlowError;

async fn call_llm() -> Result<String, DashFlowError> {
    let llm = ChatOpenAI::default();

    match llm.invoke("Hello").await {
        Ok(response) => Ok(response.content),
        Err(e) => {
            eprintln!("Error: {}", e);
            Err(e)
        }
    }
}
```

### Configuration

```rust
let llm = ChatOpenAI::default()
    .with_model("gpt-4")
    .with_temperature(0.7)
    .with_max_tokens(1000)
    .with_timeout(Duration::from_secs(30));
```

### Async/Await

All DashFlow operations are async:

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // All operations use .await
    let response = llm.invoke("Hello").await?;

    // Run operations concurrently
    let (response1, response2) = tokio::join!(
        llm.invoke("Question 1"),
        llm.invoke("Question 2")
    );

    Ok(())
}
```

## Next Steps

Now that you've built basic applications, explore more advanced topics:

- **[Core Concepts](./core-concepts.md)**: Deep dive into DashFlow architecture
- **[Language Models](../core/language-models.md)**: Learn about different LLM providers
- **[RAG Example](../examples/rag.md)**: Build retrieval-augmented generation
- **[Architecture Overview](../architecture/overview.md)**: Understand the system design

## Resources

- **API Reference**: [Rustdoc](../api/rustdoc.md)
- **Source Code**: [GitHub](https://github.com/dropbox/dTOOL/dashflow)
- **Examples**: [examples/](https://github.com/dropbox/dTOOL/dashflow/tree/main/examples)

## Tips

1. **Start Simple**: Begin with basic LLM calls before building complex chains
2. **Read Error Messages**: Rust's error messages are helpful - read them carefully
3. **Use Types**: Let Rust's type system guide you
4. **Test Incrementally**: Test each component before combining
5. **Check Examples**: Look at the examples/ directory for inspiration
