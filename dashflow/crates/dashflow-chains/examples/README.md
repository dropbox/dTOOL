# DashFlow Chains Examples

Comprehensive examples demonstrating the various chain patterns available in dashflow-chains.

## Prerequisites

All examples require an OpenAI API key:

```bash
export OPENAI_API_KEY="your-key-here"
```

## Running Examples

```bash
# Run a specific example
cargo run --package dashflow-chains --example 01_basic_llm_chain

# Run with output
cargo run --package dashflow-chains --example 01_basic_llm_chain -- --nocapture
```

## Examples Overview

### 1. Basic Chains

#### `01_basic_llm_chain.rs` - Basic LLM Chain
**Complexity:** Beginner
**Pattern:** Prompt formatting + LLM execution

The simplest chain pattern. Format a prompt template with variables and send to LLM.

**Use cases:**
- Simple question answering
- Text generation with templates
- Chatbot responses

**Key concepts:**
- `LLMChain` - basic chain structure
- `PromptTemplate` - templating with variables
- Direct LLM invocation

**Runtime:** ~3-5 seconds per query

---

#### `02_sequential_chain.rs` - Sequential Chain
**Complexity:** Intermediate
**Pattern:** Multi-step processing with named inputs/outputs

Execute multiple processing steps where each step can access outputs from all previous steps.

**Use cases:**
- Multi-stage content creation (outline → draft → final)
- Data transformation pipelines
- Validation and enrichment workflows

**Key concepts:**
- `SequentialChain` - compose multiple steps
- Named input/output variables
- Data flow validation at build time
- Accumulating outputs across steps

**Runtime:** ~1-2 seconds (no LLM calls in demo)

---

### 2. Document Processing

#### `03_stuff_documents.rs` - Stuff Documents Chain
**Complexity:** Intermediate
**Pattern:** Combine documents by concatenation

Concatenate all documents into a single prompt and process with LLM. Simplest document combination strategy.

**Use cases:**
- Summarizing small document sets
- Extracting information from multiple sources
- Document synthesis when content fits in context window

**Key concepts:**
- `StuffDocumentsChain` - document concatenation
- Document formatting and separation
- Context window management
- Additional prompt variables

**Limitations:**
- May hit token limits with many/large documents
- Use MapReduceDocumentsChain for large document sets

**Runtime:** ~5-8 seconds per summarization

---

### 3. Advanced Retrieval

#### `04_hyde_retrieval.rs` - HyDE (Hypothetical Document Embeddings)
**Complexity:** Advanced
**Pattern:** Generate hypothetical documents for better retrieval

Uses LLM to generate a hypothetical document that would answer a query, then embeds that document instead of the query. This can significantly improve retrieval quality.

**Based on:** [HyDE: Precise Zero-Shot Dense Retrieval without Relevance Labels](https://arxiv.org/abs/2212.10496)

**Use cases:**
- Semantic search with complex queries
- Question answering over knowledge bases
- Scientific fact verification
- Domain-specific retrieval (financial, legal, medical)

**Key concepts:**
- `HypotheticalDocumentEmbedder` - HyDE implementation
- Pre-built prompt templates for different domains
- Custom prompt templates
- Embedding comparison

**Available prompts:**
- `web_search` - General web search
- `sci_fact` - Scientific fact verification
- `fiqa` - Financial question answering
- `trec_news` - News topic exploration
- `trec_covid` - COVID research queries
- `arguana` - Counter-argument generation
- Custom prompts supported

**Runtime:** ~6-10 seconds (includes LLM call + embedding)

---

### 4. Safety and Quality

#### `05_constitutional_ai.rs` - Constitutional AI Chain
**Complexity:** Advanced
**Pattern:** Self-critique and revision based on principles

Implements Constitutional AI method where LLM critiques and revises its own outputs according to defined principles.

**Based on:** [Constitutional AI: Harmlessness from AI Feedback](https://arxiv.org/abs/2212.08073)

**Use cases:**
- Content moderation and safety
- Removing biased or harmful content
- Factuality checking
- Maintaining brand voice and guidelines
- Regulatory compliance

**Key concepts:**
- `ConstitutionalChain` - multi-stage critique/revision
- `ConstitutionalPrinciple` - define critique and revision requests
- Built-in safety principles
- Custom domain-specific principles

**Built-in principles:**
- `harmful1-4` - Detect harmful, unethical, racist, sexist, toxic content
- `insensitive` - Check for insensitive or inappropriate content
- `offensive` - Detect offensive material
- `controversial` - Identify controversial statements
- `ethical` - Ensure ethical guidelines
- `polite` - Maintain polite tone

**Trade-offs:**
- Multiple LLM calls (initial + 2 per principle)
- Higher cost but improved safety/quality
- Best for production systems requiring high standards

**Runtime:** ~15-25 seconds (3-5 LLM calls)

---

### 5. Question Answering

#### `06_retrieval_qa.rs` - Retrieval QA Chain
**Complexity:** Intermediate
**Pattern:** RAG (Retrieval Augmented Generation)

Foundation pattern for question answering over documents. Retrieves relevant documents and generates answers grounded in retrieved content.

**Use cases:**
- Knowledge base Q&A
- Documentation search
- Customer support chatbots
- Research assistance
- Fact-checking

**Key concepts:**
- `RetrievalQA` - retrieval + LLM answering
- `Retriever` interface
- Document combining strategies
- Reducing hallucination through grounding

**Chain types:**
- `Stuff` - Concatenate all docs (fast, may hit limits)
- `MapReduce` - Parallel processing, then combine (scales well)
- `Refine` - Iterative refinement (best quality)

**Architecture:**
```
Query → Retriever → Documents → Combine Docs → LLM → Answer
```

**Runtime:** ~6-10 seconds per question

---

## Chain Comparison Matrix

| Chain | Complexity | LLM Calls | Best For | Scales To |
|-------|-----------|-----------|----------|-----------|
| LLMChain | Beginner | 1 | Simple prompts | N/A |
| SequentialChain | Intermediate | N (# steps) | Multi-stage workflows | 10+ steps |
| StuffDocuments | Intermediate | 1 | Small doc sets | ~10 docs |
| MapReduceDocuments | Advanced | N+1 (N=docs) | Large doc sets | 100s of docs |
| HyDE | Advanced | 2 (gen + embed) | Complex queries | N/A |
| ConstitutionalAI | Advanced | 1 + 2N (N=principles) | Safety-critical | 5-10 principles |
| RetrievalQA | Intermediate | 1-N (depends on chain type) | Q&A over docs | 1000s of docs |

---

## Performance Tips

### Token Optimization
- Use `Stuff` chain type for small document sets (<10 docs)
- Switch to `MapReduce` for large document sets (>20 docs)
- Set appropriate `chunk_size` for text splitters

### Cost Reduction
- Use cheaper models for initial steps (`gpt-3.5-turbo-instruct`)
- Reserve `gpt-4` for final generation
- Cache embeddings when possible
- Limit retrieved document count (`k` parameter)

### Latency Optimization
- Use parallel processing (`MapReduce`)
- Stream responses when possible
- Reduce temperature for faster, more focused responses
- Set appropriate `max_tokens` limits

---

## Common Patterns

### Pattern 1: Research Assistant
```
Query → HyDE embedding → RetrievalQA → Constitutional AI → Safe Answer
```

### Pattern 2: Content Pipeline
```
Topic → LLMChain (outline) → Sequential (expand) → Constitutional (review)
```

### Pattern 3: Document Analysis
```
Docs → StuffDocuments (summarize) → LLMChain (analyze) → Output
```

---

## Error Handling

All examples include basic error handling. In production:

1. **Handle API failures:**
   - Retry transient errors with exponential backoff
   - Implement circuit breakers
   - Log failures for debugging

2. **Validate inputs:**
   - Check token counts before stuffing documents
   - Validate retriever returns non-empty results
   - Sanitize user inputs

3. **Monitor costs:**
   - Track token usage per chain execution
   - Set budget limits
   - Alert on unusual patterns

---

## Further Reading

- [DashFlow Documentation](https://dashflow.com/)
- [AI Parts Catalog](../../../docs/AI_PARTS_CATALOG.md) - Complete component reference
- [Golden Path Guide](../../../docs/GOLDEN_PATH.md) - Recommended API patterns

---

## Contributing

To add a new example:

1. Create `NN_example_name.rs` with descriptive name
2. Include comprehensive comments
3. Add example to this README
4. Test with `cargo run --package dashflow-chains --example NN_example_name`
5. Verify it runs in <30 seconds (for CI/CD)

---

**Note:** Examples use real API calls and will incur costs. Approximate cost per example run: $0.01-0.05 USD.
