# API Reference (Rustdoc)

Complete API documentation is available via Rustdoc, Rust's built-in documentation system.

## Viewing Documentation

### Local Development

Generate and open the documentation:

```bash
cargo doc --no-deps --workspace --open
```

This will:
1. Generate documentation for all crates in the workspace
2. Open the documentation in your browser
3. Include all public APIs with their documentation

### Online Documentation

**Note**: When published to crates.io, documentation will be automatically available at:
- `https://docs.rs/dashflow/latest/dashflow/`
- `https://docs.rs/dashflow-openai/latest/dashflow_openai/`
- etc.

## Core Crates Documentation

### dashflow

The foundation crate containing all core traits and abstractions.

**Key Modules:**
- `language_models` - ChatModel and LLM traits
- `embeddings` - Embeddings trait
- `vector_stores` - VectorStore trait
- `document_loaders` - Document loading (100+ loaders)
- `text_splitters` - Text splitting strategies
- `agents` - Agent framework
- `memory` - Memory systems
- `tools` - Tool execution framework
- `prompts` - Prompt templates
- `documents` - Document types
- `messages` - Message types for chat

**Generate docs:**
```bash
cargo doc -p dashflow --open
```

### dashflow-openai

OpenAI integration (ChatGPT, GPT-4, embeddings).

**Key Types:**
- `ChatOpenAI` - OpenAI chat models
- `OpenAIEmbeddings` - OpenAI embeddings
- `OpenAIConfig` - Configuration options

**Generate docs:**
```bash
cargo doc -p dashflow-openai --open
```

### dashflow-anthropic

Anthropic Claude integration.

**Key Types:**
- `ChatAnthropic` - Claude chat models
- `AnthropicEmbeddings` - Claude embeddings
- `AnthropicConfig` - Configuration

**Generate docs:**
```bash
cargo doc -p dashflow-anthropic --open
```

### dashflow-qdrant

Qdrant vector database integration.

**Key Types:**
- `QdrantVectorStore` - Qdrant vector store implementation
- `QdrantConfig` - Configuration options

**Generate docs:**
```bash
cargo doc -p dashflow-qdrant --open
```

### dashflow-text-splitters

Text splitting implementations.

**Key Types:**
- `RecursiveCharacterTextSplitter` - Recursive splitting
- `TokenTextSplitter` - Token-based splitting
- `MarkdownTextSplitter` - Markdown-aware splitting
- `CodeTextSplitter` - Code-aware splitting

**Generate docs:**
```bash
cargo doc -p dashflow-text-splitters --open
```

## Documentation Features

### Code Examples

Most public APIs include runnable examples in their documentation:

```rust
/// Generate a chat response.
///
/// # Example
///
/// ```no_run
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::language_models::ChatModel;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let llm = ChatOpenAI::default();
///     let response = llm.invoke("Hello!").await?;
///     println!("{}", response.content);
///     Ok(())
/// }
/// ```
pub async fn invoke(&self, input: &str) -> Result<ChatResponse>;
```

### Type Documentation

All public types are documented with:
- **Purpose**: What the type represents
- **Fields**: Description of each field
- **Methods**: What each method does
- **Examples**: How to use the type
- **See Also**: Related types and modules

### Trait Documentation

Traits include:
- **Overview**: What the trait represents
- **Required Methods**: Methods you must implement
- **Provided Methods**: Default implementations
- **Implementers**: Types that implement the trait
- **Examples**: How to implement and use

## Searching Documentation

### Command-Line Search

```bash
# Search for a specific item
cargo doc --open
# Then use the search bar at the top of the page
```

### Grep Through Source

```bash
# Find all implementations of a trait
rg "impl.*ChatModel" --type rust

# Find all uses of a function
rg "\.invoke\(" --type rust
```

## Documentation Coverage

Generate a coverage report:

```bash
RUSTDOCFLAGS="--show-coverage" cargo doc --no-deps --workspace
```

This shows which public items are missing documentation.

## Writing Documentation

### Adding Documentation to Your Code

```rust
/// Short one-line summary.
///
/// Longer description with details about what this does,
/// how it works, and when to use it.
///
/// # Arguments
///
/// * `input` - The input text to process
/// * `config` - Optional configuration
///
/// # Returns
///
/// Returns a `Result` containing the processed output or an error.
///
/// # Errors
///
/// Returns an error if:
/// - The input is invalid
/// - The API call fails
/// - The network is unreachable
///
/// # Example
///
/// ```no_run
/// # use dashflow::core::*;
/// let result = process_text("input", None).await?;
/// ```
pub async fn process_text(
    input: &str,
    config: Option<Config>,
) -> Result<String> {
    // Implementation
}
```

### Documentation Sections

- `# Arguments` - Parameter descriptions
- `# Returns` - Return value description
- `# Errors` - Error conditions
- `# Panics` - When the function panics
- `# Safety` - Safety requirements for `unsafe` code
- `# Examples` - Code examples
- `# See Also` - Related items

## API Stability

### Version Guarantees

Following [Semantic Versioning](https://semver.org/):

- **Major version (1.x.x → 2.x.x)**: Breaking changes allowed
- **Minor version (1.5.x → 1.6.x)**: New features, backward compatible
- **Patch version (1.5.0 → 1.5.1)**: Bug fixes only

### Deprecation

Deprecated APIs are marked:

```rust
#[deprecated(since = "1.5.0", note = "Use `new_method` instead")]
pub fn old_method() { }
```

The compiler will warn when using deprecated APIs.

## Advanced Documentation

### Linking to Other Items

```rust
/// See [`ChatModel`](crate::language_models::ChatModel) for details.
/// Also check [`ChatOpenAI`](dashflow_openai::ChatOpenAI).
```

### Including External Files

```rust
/// # Example from file
///
/// ```rust
/// # include!("../examples/basic.rs");
/// ```
```

### Hiding Lines in Examples

```rust
/// ```
/// # use dashflow::core::*;  // Hidden
/// # #[tokio::main]
/// # async fn main() {
/// let result = public_api().await;  // Visible
/// # }
/// ```
```

Lines starting with `#` are hidden in the rendered documentation but are still compiled.

## Rustdoc Themes

Rustdoc supports multiple themes:
- **Light** - Default light theme
- **Dark** - Dark theme
- **Ayu** - Ayu dark theme
- **Rust** - Rust branding theme

Toggle themes in the top-right corner of the documentation.

## Navigation Tips

### Keyboard Shortcuts

When viewing documentation:

- `S` - Focus search bar
- `?` - Show keyboard shortcuts
- `+` - Expand all sections
- `-` - Collapse all sections

### Quick Navigation

- **Search** - Use the search bar for fuzzy searching
- **Source** - Click "source" to view implementation
- **Implementations** - See all trait implementations
- **Blanket Implementations** - See automatic trait implementations

## Generating Documentation for Dependencies

Include dependencies in documentation:

```bash
cargo doc --workspace --open
```

This includes documentation for all dependencies, not just your crates.

## Related Resources

- **[Examples](../examples/rag.md)** - Practical examples
- **[Architecture Overview](../architecture/overview.md)** - System design
- **[Rust Documentation Guidelines](https://doc.rust-lang.org/rustdoc/how-to-write-documentation.html)**

## Feedback

If you find missing or unclear documentation:

1. Open an issue on [GitHub](https://github.com/dropbox/dTOOL/dashflow/issues)
2. Include:
   - What you were looking for
   - What's missing or unclear
   - Suggestions for improvement
