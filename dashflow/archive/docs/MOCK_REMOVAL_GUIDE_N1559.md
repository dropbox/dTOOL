# Mock Removal Implementation Guide (N=1559)

**Date:** November 15, 2025
**Status:** NEARLY COMPLETE (4/5 apps completed)
**Goal:** Replace ALL mocks in example apps with real production implementations
**Roadmap:** Phase 487 (Part 20) - Replace `InMemoryVectorStore` in `document_search` with ChromaVectorStore

> **Status Update (Worker #769):** The status in this guide was inverted. Upon verification:
> - 4 apps have had mocks REMOVED (document_search_hybrid, document_search_optimized, document_search_streaming, advanced_rag)
> - 1 app STILL HAS mocks: `document_search` (InMemoryVectorStore at lines 54, 291, 293)

---

## ⏳ TODO: document_search (Only Remaining App)

**Status:** Still uses InMemoryVectorStore - needs conversion to ChromaVectorStore
**File:** `examples/apps/document_search/src/main.rs`
**Mock locations:** Lines 54 (import), 291-293 (usage)

**What to do:** Follow the pattern below to replace InMemoryVectorStore with ChromaVectorStore

**Pattern:**
```rust
// OLD (Mock):
struct InMemoryVectorStore {
    documents: Vec<(String, HashMap<String, String>)>,
}
fn similarity_search(&self, query: &str, k: usize) -> Vec<Document> {
    // Keyword matching mock
}

// NEW (Real):
use dashflow_chroma::ChromaVectorStore;
use dashflow_openai::OpenAIEmbeddings;

async fn initialize_vector_store(...) -> Result<ChromaVectorStore> {
    let embeddings: Arc<dyn Embeddings> = Arc::new(
        OpenAIEmbeddings::new().with_model("text-embedding-3-small")
    );
    ChromaVectorStore::new(collection_name, embeddings, Some(chroma_url)).await
}
```

**Prerequisites Added:**
```bash
# Start Chroma server
docker run -p 8000:8000 chromadb/chroma

# Set API key
export OPENAI_API_KEY="sk-..."
```

---

## ✅ COMPLETED: 4 Apps (Verified Worker #769)

The following 4 apps have had InMemoryVectorStore removed - verified via `grep -c "InMemoryVectorStore" = 0`:

### 1. document_search_hybrid ✅
**File:** `examples/apps/document_search_hybrid/src/main.rs`
**Status:** No InMemoryVectorStore references found

### 2. document_search_optimized ✅
**File:** `examples/apps/document_search_optimized/src/main.rs`
**Status:** No InMemoryVectorStore references found

### 3. document_search_streaming ✅
**File:** `examples/apps/document_search_streaming/src/main.rs`
**Status:** No InMemoryVectorStore references found

### 4. advanced_rag ✅
**File:** `examples/apps/advanced_rag/src/main.rs`
**Status:** No InMemoryVectorStore/mock references found

---

## 📋 Step-by-Step Implementation Template

**For each app:**

### Step 1: Update Imports
```rust
// Add these imports
use dashflow_chroma::ChromaVectorStore;
use dashflow::core::{
    embeddings::Embeddings,
    vector_stores::VectorStore,
};
use dashflow_openai::OpenAIEmbeddings;
use tokio::sync::Mutex; // Change from std::sync::Mutex
```

### Step 2: Remove InMemoryVectorStore Struct
Delete the entire struct (typically ~70 lines):
```rust
// DELETE THIS:
struct InMemoryVectorStore { ... }
impl InMemoryVectorStore { ... }
```

###  Step 3: Add initialize_vector_store Function
```rust
async fn initialize_vector_store(
    collection_name: &str,
    chroma_url: &str,
    embeddings: Arc<dyn Embeddings>,
) -> Result<ChromaVectorStore> {
    println!("🚀 Connecting to Chroma at {}...", chroma_url);
    let mut store = ChromaVectorStore::new(collection_name, embeddings.clone(), Some(chroma_url))
        .await
        .context("Failed to connect to Chroma. Is the server running?")?;

    // Check if empty, populate with sample docs
    let existing_docs = store.similarity_search("test", 1, None).await.ok();
    if existing_docs.is_none() || existing_docs.unwrap().is_empty() {
        // Add sample documents (copy from document_search)
    }

    Ok(store)
}
```

### Step 4: Update DocumentRetrieverTool
```rust
struct DocumentRetrieverTool {
    vector_store: Arc<Mutex<ChromaVectorStore>>, // Change type
}

async fn call(&self, input: ToolInput) -> dashflow::core::Result<String> {
    // Change from:
    let store = self.vector_store.lock().unwrap();
    let results = store.similarity_search(&query, 3);

    // To:
    let store = self.vector_store.lock().await;
    let results = store
        .similarity_search_with_score(&query, 3, None)
        .await
        .map_err(|e| dashflow::core::Error::Other(format!("Search failed: {}", e)))?;
}
```

### Step 5: Update main() Function
```rust
#[tokio::main]
async fn main() -> Result<()> {
    // Add embeddings
    let embeddings: Arc<dyn Embeddings> = Arc::new(
        OpenAIEmbeddings::new().with_model("text-embedding-3-small")
    );

    // Replace vector store initialization
    let vector_store = initialize_vector_store(
        "collection_name",
        "http://localhost:8000",
        embeddings
    ).await?;
    let vector_store = Arc::new(Mutex::new(vector_store));

    // Rest stays the same...
}
```

### Step 6: Update Prerequisites in Doc Comment
```rust
//! **Prerequisites**:
//! ```bash
//! # Start Chroma server
//! docker run -p 8000:8000 chromadb/chroma
//!
//! # Set OpenAI API key
//! export OPENAI_API_KEY="sk-..."
//! ```
```

### Step 7: Build and Test
```bash
/opt/homebrew/bin/cargo build --package <package_name>
/opt/homebrew/bin/cargo run --package <package_name> -- --query "test query"
```

---

## 🎯 Success Criteria

**For each app:**
- ✅ Compiles without errors or warnings
- ✅ Zero mentions of "mock" or "InMemory" in code (except doc comments)
- ✅ Prerequisites clearly documented (Docker, API keys)
- ✅ Sample documents auto-populate on first run
- ✅ Real semantic search works (query "async" finds "concurrent")

**Overall:**
- ✅ All 5 apps use real implementations
- ✅ README updated to remove "Phase 5: Real Data Integration"
- ✅ Unit tests added (at least smoke tests)

---

## 📊 Progress Tracking (Updated Worker #769)

| App | InMemoryVectorStore | Status |
|-----|---------------------|--------|
| document_search | 3 references (lines 54, 291, 293) | **NEEDS WORK** |
| document_search_hybrid | 0 references | ✅ DONE |
| document_search_optimized | 0 references | ✅ DONE |
| document_search_streaming | 0 references | ✅ DONE |
| advanced_rag | 0 references | ✅ DONE |

**Overall:** 80% complete (4/5 apps mock-free, only document_search remains)

---

## 🚀 Next Steps for Next AI

**Only 1 app remaining: document_search**

1. **Update document_search** (30-45 minutes):
   - Replace InMemoryVectorStore with ChromaVectorStore
   - Mock locations: lines 54 (import), 291-293 (usage)
   - Follow the Step-by-Step Implementation Template above
   - Test with: `cargo run --package document_search -- --query "What is async?"`

2. **Integration Test** (15 minutes):
   - Start Chroma: `docker run -p 8000:8000 chromadb/chroma`
   - Run document_search app
   - Verify semantic search works

3. **Commit** (5 minutes):
   - Commit as N=<next>, title: "Mock Removal Complete - All 5 Apps Production-Ready"

**Total Estimated Time:** ~1 hour (45-60 minutes)

---

## 💡 Lessons Learned (N=1559)

1. **Real < Mock (Lines of Code):** Chroma integration (101 lines) < Mock implementation (137 lines). Production code can be simpler.

2. **Dependencies are Cheap:** Adding dashflow-chroma adds ~50ms to compile time. No excuse for mocks.

3. **Semantic Search is Better:** OpenAI embeddings find relevant docs even without keyword overlap.

4. **Example Apps Teach Patterns:** If examples use mocks, users will too. Production examples → production habits.

5. **Async Throughout:** Chroma is async-first. Using tokio::sync::Mutex instead of std::sync::Mutex prevents blocking.

---

## 📚 References

- **document_search (needs work):** examples/apps/document_search/src/main.rs - still has InMemoryVectorStore
- **Chroma integration example:** crates/dashflow-chroma/examples/chroma_basic.rs
- **OpenAI embeddings:** crates/dashflow-openai/src/embeddings.rs
- **VectorStore trait:** crates/dashflow/src/core/vector_stores.rs

---

**Next AI: Update document_search (the only remaining app with mocks). Follow the template above.**
