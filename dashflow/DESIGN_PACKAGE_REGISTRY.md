# DashBoard: Package Registry Implementation

**Version:** 1.0
**Date:** 2025-12-09
**Status:** DESIGN
**Parent:** DESIGN_PACKAGE_ECOSYSTEM.md
**Platform:** DashBoard (GitHub + Jira + Marketplace for AI Agents)

---

## DashBoard Vision

DashBoard is the unified platform for distributed agentic engineering:
- **GitHub-like**: Git hosting, code review, PRs
- **Jira-like**: Work items, sprints, agent assignments
- **Marketplace**: Package discovery, distribution, contributions

The package registry is one component of DashBoard, not a standalone service.

```
DashBoard Platform
├── /packages     ← Package Marketplace (this document)
├── /projects     ← Project Management (Jira-like)
├── /repos        ← Git Hosting (or links to GitHub/GitLab)
├── /agents       ← Agent Registry & Coordination
└── /trust        ← Signatures, Keys, Verification
```

---

## Git Integration

**Git is the source control layer.** DashBoard doesn't replace git, it builds on top:

- Package source lives in git repos (GitHub, GitLab, self-hosted, or DashBoard-hosted)
- Publishing creates a snapshot in the registry (content-addressed)
- Full git history preserved and browseable
- Existing CI/CD workflows continue to work

```
Git Repo (source)                    DashBoard Registry (distribution)
─────────────────                    ─────────────────────────────────
github.com/user/pkg                  dashboard.com/packages/user/pkg
    │                                         │
    │  git tag v1.2.0                         │  Package v1.2.0
    │  dashflow.toml    ──── publish ────►    │  hash: sha256:...
    │                                         │  signed: ✓
    └── full history                          └── searchable, P2P cacheable
```

---

## The Problem with GitHub Alone

GitHub is designed for human developers:
- PRs require web UI or complex git operations
- Issues are free-form text, not structured data
- Search is keyword-based, not semantic
- No native support for package manifests
- No cryptographic verification built-in
- No P2P distribution
- Rate limits hostile to AI agents

**We need an AI-native registry from the ground up.**

---

## Design Principles

1. **API-First**: Every operation is an API call, not a git command or web form
2. **Structured Data**: Contributions are typed schemas, not free-form text
3. **Content-Addressed**: Packages identified by hash, not name+version
4. **Colony-First**: Local sharing before internet fetch
5. **Semantic-Native**: Vector search is primary, keyword is fallback
6. **Trust-Native**: Signatures verified at every layer
7. **AI-Optimized**: Batch operations, rate limits designed for agents

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    DashFlow Package Registry Architecture                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        dashswarm.com                                    │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐                  │ │
│  │  │   API        │  │   Search     │  │   Trust      │                  │ │
│  │  │   Gateway    │  │   Service    │  │   Service    │                  │ │
│  │  │   (REST/     │  │   (Vector +  │  │   (Sig       │                  │ │
│  │  │   GraphQL)   │  │   Keyword)   │  │   Verify)    │                  │ │
│  │  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘                  │ │
│  │         │                 │                 │                           │ │
│  │         └─────────────────┼─────────────────┘                           │ │
│  │                           ▼                                             │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │ │
│  │  │                     Content Store (CAS)                           │  │ │
│  │  │  • Packages stored by content hash (SHA-256)                     │  │ │
│  │  │  • Deduplication automatic                                        │  │ │
│  │  │  • CDN-distributed (Cloudflare R2 / S3)                          │  │ │
│  │  └──────────────────────────────────────────────────────────────────┘  │ │
│  │                           │                                             │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │ │
│  │  │                     Metadata Store                                │  │ │
│  │  │  • Package manifests (PostgreSQL)                                │  │ │
│  │  │  • Version history                                                │  │ │
│  │  │  • Lineage/derivation chains                                     │  │ │
│  │  │  • Contribution records                                           │  │ │
│  │  └──────────────────────────────────────────────────────────────────┘  │ │
│  │                           │                                             │ │
│  │  ┌──────────────────────────────────────────────────────────────────┐  │ │
│  │  │                     Vector Store                                  │  │ │
│  │  │  • Package embeddings (Qdrant/Pinecone)                          │  │ │
│  │  │  • Semantic search index                                          │  │ │
│  │  │  • Capability matching                                            │  │ │
│  │  └──────────────────────────────────────────────────────────────────┘  │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                    ▲                                         │
│                                    │ sync                                    │
│                                    ▼                                         │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                        Colony Mesh (P2P)                               │ │
│  │                                                                         │ │
│  │    ┌─────────┐      ┌─────────┐      ┌─────────┐                       │ │
│  │    │  App A  │◄────►│  App B  │◄────►│  App C  │                       │ │
│  │    │  Cache  │      │  Cache  │      │  Cache  │                       │ │
│  │    └─────────┘      └─────────┘      └─────────┘                       │ │
│  │                                                                         │ │
│  │    Local packages shared before fetching from central registry         │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Content-Addressed Storage (CAS)

Every package is stored by its content hash, not by name+version.

```rust
/// A package in the content store
pub struct StoredPackage {
    /// SHA-256 hash of the package tarball
    pub hash: ContentHash,

    /// Size in bytes
    pub size: u64,

    /// MIME type
    pub content_type: String,

    /// Storage locations (redundant)
    pub locations: Vec<StorageLocation>,
}

pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    pub fn from_bytes(data: &[u8]) -> Self {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(data);
        Self(hasher.finalize().into())
    }

    pub fn to_string(&self) -> String {
        format!("sha256:{}", hex::encode(self.0))
    }
}

pub enum StorageLocation {
    /// Primary CDN (Cloudflare R2)
    Cdn { url: String, region: String },

    /// Colony peer cache
    ColonyPeer { app_id: Uuid, endpoint: String },

    /// IPFS (future)
    Ipfs { cid: String },
}
```

### Benefits of CAS

1. **Deduplication**: Same content = same hash = stored once
2. **Verification**: Download and verify hash matches
3. **Caching**: Colony peers cache by hash, not name
4. **Immutability**: Content at a hash never changes
5. **Distribution**: Any peer with the hash can serve it

---

## API Design (AI-Native)

### Core Principles

1. **Structured requests**: JSON schemas, not form data
2. **Batch operations**: Multiple operations in one request
3. **Async by default**: Long operations return job IDs
4. **Machine-readable errors**: Error codes, not prose
5. **Rate limit friendly**: Generous limits, backoff headers

### API Endpoints

```
# Package Operations
POST   /api/v1/packages                    # Publish package
GET    /api/v1/packages/:hash              # Get by content hash
GET    /api/v1/packages/resolve/:name/:version  # Resolve name → hash
DELETE /api/v1/packages/:hash              # Yank (mark unavailable)

# Search
POST   /api/v1/search                      # Unified search
POST   /api/v1/search/semantic             # Semantic only (embedding)
GET    /api/v1/search/keyword              # Keyword only
POST   /api/v1/search/capability           # Find by capability

# Contributions (structured, not free-form)
POST   /api/v1/contributions/bug           # Structured bug report
POST   /api/v1/contributions/improvement   # Structured improvement
POST   /api/v1/contributions/request       # Structured package request
POST   /api/v1/contributions/fix           # Structured fix submission
GET    /api/v1/contributions/:id           # Get contribution status
POST   /api/v1/contributions/:id/review    # Submit review (AI or human)

# Trust
POST   /api/v1/trust/verify                # Verify signature
GET    /api/v1/trust/keys                  # List trusted keys
GET    /api/v1/trust/lineage/:hash         # Get derivation chain

# Batch Operations (AI-optimized)
POST   /api/v1/batch                       # Multiple operations
POST   /api/v1/batch/resolve               # Resolve multiple packages
POST   /api/v1/batch/download              # Get download URLs for multiple

# Colony Sync
POST   /api/v1/colony/sync                 # Sync colony state
GET    /api/v1/colony/peers/:package       # Find peers with package
```

### Example: Structured Bug Report

Instead of free-form GitHub issue:

```json
POST /api/v1/contributions/bug
{
  "package_hash": "sha256:abc123...",
  "reporter": {
    "app_id": "uuid",
    "app_name": "CustomerServiceBot",
    "public_key": "ed25519:...",
    "is_ai": true
  },
  "bug": {
    "category": "runtime_error",
    "severity": "medium",
    "title": "Fails on emoji input",
    "description": "SentimentNode throws ParseError when input contains emoji",
    "trigger_conditions": [
      { "field": "input.length", "operator": ">", "value": 0 },
      { "field": "input.contains_emoji", "operator": "==", "value": true }
    ],
    "occurrence_rate": 0.03,
    "sample_count": 10000
  },
  "evidence": {
    "traces": ["trace-id-1", "trace-id-2"],
    "error_messages": ["ParseError: unexpected character at position 47"],
    "reproduction_steps": [
      { "action": "create_node", "params": { "type": "SentimentNode" } },
      { "action": "invoke", "params": { "input": "Great service! 😊" } }
    ]
  },
  "suggested_fix": {
    "description": "Add emoji handling to tokenizer",
    "confidence": 0.75,
    "diff": "--- a/src/tokenizer.rs\n+++ b/src/tokenizer.rs\n..."
  },
  "signature": "ed25519:..."
}
```

Response:
```json
{
  "contribution_id": "uuid",
  "status": "submitted",
  "validation": {
    "schema_valid": true,
    "signature_valid": true,
    "evidence_verifiable": true
  },
  "next_steps": [
    "awaiting_consensus_review",
    "estimated_review_time_hours": 24
  ]
}
```

---

## Search Architecture

### Unified Search

Combines semantic, keyword, and capability matching:

```rust
pub struct SearchRequest {
    /// Natural language query (semantic search)
    pub query: Option<String>,

    /// Keywords (keyword search)
    pub keywords: Option<Vec<String>>,

    /// Required capabilities (capability search)
    pub capabilities: Option<Vec<Capability>>,

    /// Filters
    pub filters: SearchFilters,

    /// Pagination
    pub limit: u32,
    pub offset: u32,
}

pub struct SearchFilters {
    pub package_type: Option<PackageType>,
    pub min_downloads: Option<u64>,
    pub verified_only: bool,
    pub min_trust_level: Option<TrustLevel>,
    pub updated_after: Option<DateTime<Utc>>,
    pub namespace: Option<String>,
}

pub struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: u64,

    /// Which search methods contributed
    pub sources: SearchSources,
}

pub struct SearchResult {
    pub package: PackageInfo,

    /// Combined relevance score (0-1)
    pub score: f64,

    /// Score breakdown
    pub score_components: ScoreComponents,

    /// Why this matched
    pub match_reasons: Vec<MatchReason>,
}

pub struct ScoreComponents {
    pub semantic_score: Option<f64>,    // Vector similarity
    pub keyword_score: Option<f64>,     // BM25
    pub capability_score: Option<f64>,  // Capability match
    pub popularity_score: f64,          // Downloads, usage
    pub trust_score: f64,               // Verification status
}
```

### Semantic Search Implementation

```rust
pub struct SemanticSearchService {
    /// Embedding model (local or API)
    embedder: Box<dyn Embedder>,

    /// Vector database
    vector_db: VectorDb,
}

impl SemanticSearchService {
    /// Index a package
    pub async fn index(&self, package: &PackageInfo) -> Result<()> {
        // Create rich text representation
        let text = format!(
            "{name} - {description}\n\nKeywords: {keywords}\n\nCapabilities: {capabilities}\n\n{readme}",
            name = package.name,
            description = package.description,
            keywords = package.keywords.join(", "),
            capabilities = package.provides.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", "),
            readme = package.readme.as_deref().unwrap_or(""),
        );

        // Generate embedding
        let embedding = self.embedder.embed(&text).await?;

        // Store in vector DB
        self.vector_db.upsert(
            &package.hash.to_string(),
            embedding,
            PackageMetadata::from(package),
        ).await
    }

    /// Search semantically
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SemanticMatch>> {
        let query_embedding = self.embedder.embed(query).await?;

        self.vector_db.search(query_embedding, limit).await
    }
}

/// Embedder trait - can use local model or API
pub trait Embedder: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
}

/// Local embedder using ONNX
pub struct LocalEmbedder {
    model: ort::Session,
    tokenizer: tokenizers::Tokenizer,
}

/// API embedder (OpenAI, Cohere, etc.)
pub struct ApiEmbedder {
    client: reqwest::Client,
    endpoint: String,
    api_key: String,
}
```

---

## Colony P2P Distribution

Before fetching from central registry, check colony peers.

```rust
pub struct ColonyPackageResolver {
    /// Central registry client
    registry: RegistryClient,

    /// Colony network
    colony: ColonyNetwork,

    /// Local cache
    cache: PackageCache,
}

impl ColonyPackageResolver {
    /// Resolve and fetch a package
    pub async fn fetch(&self, name: &str, version: &VersionReq) -> Result<PackageData> {
        // 1. Check local cache
        if let Some(cached) = self.cache.get(name, version)? {
            return Ok(cached);
        }

        // 2. Resolve name → hash from registry (lightweight API call)
        let resolution = self.registry.resolve(name, version).await?;
        let hash = resolution.hash;

        // 3. Check if any colony peer has this hash
        let colony_peers = self.colony.peers_with_package(&hash).await;

        if !colony_peers.is_empty() {
            // 4a. Fetch from colony peer (fast, free)
            for peer in colony_peers {
                match self.fetch_from_peer(&peer, &hash).await {
                    Ok(data) => {
                        // Verify hash
                        if data.verify_hash(&hash)? {
                            self.cache.store(&hash, &data)?;
                            return Ok(data);
                        }
                    }
                    Err(_) => continue, // Try next peer
                }
            }
        }

        // 4b. Fetch from central registry (slower, uses bandwidth quota)
        let data = self.registry.download(&hash).await?;

        // Verify and cache
        if data.verify_hash(&hash)? {
            self.cache.store(&hash, &data)?;

            // Announce to colony that we have this package
            self.colony.announce_package(&hash).await?;

            Ok(data)
        } else {
            Err(PackageError::HashMismatch)
        }
    }

    async fn fetch_from_peer(&self, peer: &PeerInfo, hash: &ContentHash) -> Result<PackageData> {
        let request = PackageTransferRequest {
            hash: hash.clone(),
            requester: self.colony.identity().id,
        };

        let response = self.colony.request(
            peer.app_id,
            "_package_transfer",
            request
        ).await?;

        match response {
            PackageTransferResponse::Data(data) => Ok(data),
            PackageTransferResponse::Redirect(url) => {
                // Peer says "get it from here instead"
                self.download_from_url(&url).await
            }
            PackageTransferResponse::Denied(reason) => {
                Err(PackageError::TransferDenied(reason))
            }
        }
    }
}
```

---

## Contribution Review System

Multi-model consensus for AI contributions:

```rust
pub struct ContributionReviewer {
    /// Review models
    reviewers: Vec<Box<dyn ModelReviewer>>,

    /// Consensus threshold
    config: ReviewConfig,
}

pub struct ReviewConfig {
    /// Minimum models that must review
    pub min_reviews: usize,

    /// Consensus score threshold (0-1)
    pub consensus_threshold: f64,

    /// Auto-approve if consensus reached?
    pub auto_approve: AutoApprovePolicy,
}

pub enum AutoApprovePolicy {
    /// Never auto-approve, always human review
    Never,

    /// Auto-approve low-risk contributions
    LowRisk {
        max_lines_changed: usize,
        allowed_types: Vec<ContributionType>,
    },

    /// Auto-approve if high consensus
    HighConsensus {
        threshold: f64,  // e.g., 0.95
    },

    /// Auto-approve all with consensus
    Always,
}

impl ContributionReviewer {
    pub async fn review(&self, contribution: &Contribution) -> Result<ReviewResult> {
        // Get reviews from multiple models
        let reviews: Vec<ModelReviewResult> = futures::future::join_all(
            self.reviewers.iter().map(|r| r.review(contribution))
        ).await.into_iter().collect::<Result<Vec<_>>>()?;

        // Calculate consensus
        let consensus = self.calculate_consensus(&reviews);

        // Determine action
        let action = match &self.config.auto_approve {
            AutoApprovePolicy::Never => ReviewAction::RequireHumanApproval,

            AutoApprovePolicy::LowRisk { max_lines_changed, allowed_types } => {
                if contribution.lines_changed() <= *max_lines_changed
                    && allowed_types.contains(&contribution.contribution_type())
                    && consensus.score >= self.config.consensus_threshold
                {
                    ReviewAction::AutoApprove
                } else {
                    ReviewAction::RequireHumanApproval
                }
            }

            AutoApprovePolicy::HighConsensus { threshold } => {
                if consensus.score >= *threshold {
                    ReviewAction::AutoApprove
                } else if consensus.score >= self.config.consensus_threshold {
                    ReviewAction::NotifyHuman { urgency: Urgency::Low }
                } else {
                    ReviewAction::RequireHumanApproval
                }
            }

            AutoApprovePolicy::Always => {
                if consensus.score >= self.config.consensus_threshold {
                    ReviewAction::AutoApprove
                } else {
                    ReviewAction::Reject { reasons: consensus.disagreements }
                }
            }
        };

        Ok(ReviewResult {
            reviews,
            consensus,
            action,
        })
    }
}
```

---

## Trust Verification

Every package operation verifies signatures:

```rust
pub struct TrustService {
    /// Known public keys
    keyring: Keyring,

    /// Signature verifier
    verifier: SignatureVerifier,
}

impl TrustService {
    /// Verify a package signature
    pub fn verify_package(&self, package: &SignedPackage) -> Result<VerificationResult> {
        let mut results = Vec::new();

        for signature in &package.signatures {
            // Look up the key
            let key = self.keyring.get(&signature.key_id)?;

            // Verify the signature
            let valid = self.verifier.verify(
                &signature.signature,
                &signature.signed_content,
                &key.public_key,
            )?;

            results.push(SignatureVerification {
                key_id: signature.key_id.clone(),
                key_owner: key.owner.clone(),
                trust_level: key.trust_level,
                valid,
                timestamp: signature.timestamp,
            });
        }

        // Determine overall trust level
        let trust_level = self.calculate_trust_level(&results);

        Ok(VerificationResult {
            signatures: results,
            trust_level,
            verified: trust_level >= TrustLevel::Community,
        })
    }

    /// Verify lineage chain
    pub fn verify_lineage(&self, lineage: &Lineage) -> Result<LineageVerification> {
        let mut chain_valid = true;
        let mut step_results = Vec::new();

        for step in &lineage.chain {
            // Each step must be signed by a trusted key
            let step_valid = self.verify_derivation_step(step)?;
            chain_valid = chain_valid && step_valid.valid;
            step_results.push(step_valid);
        }

        Ok(LineageVerification {
            chain_valid,
            steps: step_results,
            original: lineage.derived_from.clone(),
        })
    }
}
```

---

## Fractal Swarm Architecture

Swarms are fractal - same structure at different scales, different trust boundaries.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         Fractal Swarm Hierarchy                              │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  PERSONAL SWARM (your machine)                                              │
│  Trust: Implicit     Data: Everything     Packages: Local cache             │
│                                                                              │
│          ▲ federate                                                         │
│          ▼                                                                   │
│                                                                              │
│  TEAM SWARM (LAN / VPN)                                                     │
│  Trust: Team keys    Data: Status, resources   Packages: Team registry      │
│                                                                              │
│          ▲ federate                                                         │
│          ▼                                                                   │
│                                                                              │
│  ORG SWARM (company-wide)                                                   │
│  Trust: Org PKI      Data: Aggregates         Packages: Private registry    │
│                                                                              │
│          ▲ federate (policy-controlled)                                     │
│          ▼                                                                   │
│                                                                              │
│  GLOBAL SWARM (dashswarm.com)                                               │
│  Trust: Signatures   Data: Public only        Packages: Marketplace         │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Same Structure at Every Level

Each swarm level has the same API:

```
Any Swarm (personal, team, org, global)
├── /packages     - Package registry for this scope
├── /agents       - Known agents in this scope
├── /work         - Work items for this scope
├── /metrics      - Metrics visible in this scope
└── /trust        - Trust roots for this scope
```

### What Differs by Level

| Layer | Trust Model | Live Metrics | Traces | Packages |
|-------|-------------|--------------|--------|----------|
| Personal | Implicit | ✓ Full | ✓ Full | Cache |
| Team | Team keys | ✓ Shared | Limited | Team registry |
| Org | Org PKI | Aggregates | None | Private registry |
| Global | Signatures | ✗ None | ✗ None | Public marketplace |

### Data Scoping

```rust
pub enum DataScope {
    /// Never leaves your machine
    Local,      // Live state, full traces, secrets, raw LLM data

    /// Shared within trusted colony
    Colony,     // Status, resources, package cache, aggregate metrics

    /// Published to global swarm
    Global,     // Package manifests, signed contributions, public keys
}
```

Code that works with your local swarm also works with the global swarm -
same interfaces, different scope.

---

## Implementation Stack

### Backend Services (Rust)

```
dashswarm-registry/
├── api-gateway/          # Axum HTTP server
│   ├── routes/
│   │   ├── packages.rs
│   │   ├── search.rs
│   │   ├── contributions.rs
│   │   └── trust.rs
│   └── middleware/
│       ├── auth.rs
│       ├── rate_limit.rs
│       └── signature_verify.rs
│
├── search-service/       # Semantic + keyword search
│   ├── embedder/
│   │   ├── local.rs      # ONNX model
│   │   └── api.rs        # OpenAI/Cohere fallback
│   ├── vector_db/
│   │   └── qdrant.rs
│   └── keyword/
│       └── meilisearch.rs
│
├── trust-service/        # Signature verification
│   ├── keyring.rs
│   ├── verifier.rs
│   └── lineage.rs
│
├── content-store/        # CAS package storage
│   ├── s3.rs             # Cloudflare R2 / S3
│   ├── cache.rs          # Redis cache
│   └── hash.rs
│
└── metadata-store/       # PostgreSQL
    ├── packages.rs
    ├── versions.rs
    ├── contributions.rs
    └── migrations/
```

### Storage

| Component | Technology | Purpose |
|-----------|------------|---------|
| Metadata | PostgreSQL | Package manifests, versions, contributions |
| Content | Cloudflare R2 | Package tarballs (CAS) |
| Vectors | Qdrant | Semantic search embeddings |
| Keywords | Meilisearch | Full-text search |
| Cache | Redis | Hot package metadata |

### Why These Choices

1. **PostgreSQL**: Battle-tested, complex queries for lineage/dependencies
2. **Cloudflare R2**: S3-compatible, global CDN, no egress fees
3. **Qdrant**: Open source, Rust-native, self-hostable
4. **Meilisearch**: Fast, typo-tolerant, developer-friendly
5. **Redis**: Caching layer for frequent metadata lookups

---

## CLI Client

```rust
/// DashFlow package CLI
pub struct PkgCli {
    registry: RegistryClient,
    colony: ColonyClient,
    cache: PackageCache,
    signer: PackageSigner,
}

impl PkgCli {
    /// dashflow pkg install sentiment-analysis
    pub async fn install(&self, spec: &str) -> Result<()> {
        let (name, version) = parse_package_spec(spec)?;

        println!("Resolving {}...", name);
        let resolution = self.registry.resolve(&name, &version).await?;

        println!("Checking colony cache...");
        let data = self.fetch_with_colony(&resolution.hash).await?;

        println!("Verifying signature...");
        self.verify(&data)?;

        println!("Installing...");
        self.cache.install(&data)?;

        println!("✓ Installed {} @ {}", name, resolution.version);
        Ok(())
    }

    /// dashflow pkg search "analyze customer sentiment"
    pub async fn search(&self, query: &str, semantic: bool) -> Result<()> {
        let results = if semantic {
            self.registry.search_semantic(query).await?
        } else {
            self.registry.search_keyword(query).await?
        };

        for result in results {
            println!(
                "{}/{} v{} - {} (score: {:.2})",
                result.package.namespace,
                result.package.name,
                result.package.version,
                result.package.description,
                result.score
            );
        }
        Ok(())
    }

    /// dashflow pkg publish ./my-package
    pub async fn publish(&self, path: &Path) -> Result<()> {
        println!("Reading manifest...");
        let manifest = PackageManifest::from_path(path)?;

        println!("Building package...");
        let tarball = self.build_tarball(path)?;
        let hash = ContentHash::from_bytes(&tarball);

        println!("Signing...");
        let signature = self.signer.sign(&hash, &manifest)?;

        println!("Uploading...");
        self.registry.publish(&manifest, &tarball, &signature).await?;

        println!("✓ Published {}@{} ({})", manifest.name, manifest.version, hash);
        Ok(())
    }

    /// dashflow pkg report-bug sentiment-analysis
    pub async fn report_bug(&self, package: &str) -> Result<()> {
        // Interactive or from file
        let report = self.gather_bug_report(package)?;

        println!("Signing report...");
        let signed = self.signer.sign_contribution(&report)?;

        println!("Submitting...");
        let id = self.registry.submit_bug_report(signed).await?;

        println!("✓ Bug report submitted: {}", id);
        Ok(())
    }
}
```

---

## Migration from GitHub

For existing packages on GitHub:

```rust
/// Import packages from GitHub
pub struct GitHubImporter {
    registry: RegistryClient,
    github: GitHubClient,
}

impl GitHubImporter {
    /// Import a GitHub release as a DashFlow package
    pub async fn import_release(
        &self,
        repo: &str,
        tag: &str,
        manifest_override: Option<PackageManifest>,
    ) -> Result<ContentHash> {
        // Fetch release tarball from GitHub
        let tarball = self.github.download_release(repo, tag).await?;

        // Generate or use provided manifest
        let manifest = match manifest_override {
            Some(m) => m,
            None => self.generate_manifest_from_cargo_toml(&tarball)?,
        };

        // Publish to DashFlow registry
        let hash = self.registry.publish(&manifest, &tarball, &self.sign()?).await?;

        Ok(hash)
    }

    /// Sync GitHub issues as structured contributions
    pub async fn import_issues(&self, repo: &str, package_hash: &ContentHash) -> Result<Vec<Uuid>> {
        let issues = self.github.list_issues(repo).await?;

        let mut contribution_ids = Vec::new();

        for issue in issues {
            // Parse issue into structured format
            let contribution = self.parse_issue_to_contribution(&issue)?;

            // Submit to registry
            let id = self.registry.submit_contribution(contribution).await?;
            contribution_ids.push(id);
        }

        Ok(contribution_ids)
    }
}
```

---

## Implementation Phases

| Phase | Commits | Focus |
|-------|---------|-------|
| 11.1 | N=354-356 | Core registry API + CAS storage |
| 11.2 | N=357-359 | Semantic search + vector DB |
| 11.3 | N=360-362 | Trust service + signature verification |
| 11.4 | N=363-365 | CLI client + local cache |
| 11.5 | N=366-368 | Colony P2P distribution |
| 11.6 | N=369-371 | Contribution system + review |
| 11.7 | N=372-374 | HTTP API gateway + PostgreSQL |
| 11.8 | N=573 | RegistryClient + CLI integration |
| 12.1 | N=574 | API route wiring to metadata store |
| 12.2 | N=574 | ContributionStore trait + PostgreSQL implementation |
| **12.3** | N=574-575 | **Contribution route wiring (COMPLETE)** |
| **12.4** | N=576 | **API key verification against database (COMPLETE)** |
| **12.5** | N=577 | **E2E integration testing (COMPLETE)** |
| **12.6** | N=578 | **Production deployment preparation (COMPLETE)** |
| 13 | TBD | Production deployment to dashswarm.com (requires server access) |
| **14** | N=579 | **CLI Registry Integration (COMPLETE)** |
| **15.1** | N=580 | **Production semantic search adapters - OpenAI + Qdrant (COMPLETE)** |
| **15.2** | N=580 | **Wire semantic search to API server (COMPLETE)** |
| **15.3** | N=580 | **Update CLI to use semantic search (COMPLETE)** |
| **16** | N=581 | **Redis caching layer - API key verification caching (COMPLETE)** |
| **17** | N=582 | **Extended caching - Package resolution + search results caching (COMPLETE)** |
| **18** | N=583 | **S3-compatible storage backend - AWS S3, R2, MinIO support (COMPLETE)** |
| **19** | N=584 | **Wire S3Storage to API server - Read-through cache pattern (COMPLETE)** |
| **20** | N=585 | **Prometheus metrics integration - HTTP, cache, storage, search metrics (COMPLETE)** |
| **21** | N=586 | **Grafana registry dashboard - Production monitoring visualization (COMPLETE)** |
| **22** | N=586 | **Prometheus alert rules - SLO/SLA alerting for registry (COMPLETE)** |
| **23** | N=586 | **Batch resolve caching - Improved performance for CI/CD pipelines (COMPLETE)** |
| **24** | N=587 | **OpenTelemetry distributed tracing - OTLP export for production observability (COMPLETE)** |
| **25** | N=588 | **CDN Integration - Presigned URLs for direct S3/R2 downloads (COMPLETE)** |

---

## Why Not Alternatives?

### Why not crates.io model?
- No semantic search
- No AI contribution support
- No P2P distribution
- No multi-model review

### Why not IPFS?
- Complexity overhead
- Reliability concerns
- Our CAS gives same benefits with simpler ops

### Why not blockchain for trust?
- Overkill for package signatures
- Ed25519 signatures are sufficient
- Faster, simpler, no gas fees

### Why not pure P2P (no central registry)?
- Discovery is hard without central index
- Semantic search needs central embedding index
- Trust/verification needs authoritative key list
- Colony P2P is additive, not replacement

---

## Version History

| Date | Change | Author |
|------|--------|--------|
| 2025-12-09 | Initial design | MANAGER |
| 2025-12-14 | Phase 11 complete, Phase 12 scope defined | Worker #574 |
| 2025-12-14 | Phase 12.3 complete - All contribution routes wired | Worker #575 |
| 2025-12-14 | Phase 12.4 complete - API key verification against database | Worker #576 |
| 2025-12-14 | Phase 12.5 complete - E2E integration testing | Worker #577 |
| 2025-12-14 | Phase 12.6 complete - Production deployment preparation | Worker #578 |
| 2025-12-14 | Phase 14 complete - CLI Registry Integration | Worker #579 |
| 2025-12-14 | Phase 15.1 complete - Production semantic search adapters (OpenAI + Qdrant) | Worker #580 |
| 2025-12-14 | Phase 16 complete - Redis caching layer for API key verification | Worker #581 |
| 2025-12-14 | Phase 17 complete - Extended caching for resolution and search | Worker #582 |
| 2025-12-14 | Phase 18 complete - S3-compatible storage backend (AWS S3, R2, MinIO) | Worker #583 |
| 2025-12-14 | Phase 19 complete - Wire S3Storage to API server with read-through caching | Worker #584 |
| 2025-12-14 | Phase 20 complete - Prometheus metrics integration | Worker #585 |
| 2025-12-14 | Phase 21 complete - Grafana registry dashboard for production monitoring | Worker #586 |
| 2025-12-14 | Phase 22 complete - Prometheus alert rules for registry SLOs | Worker #586 |
| 2025-12-14 | Phase 23 complete - Batch resolve caching for CI/CD pipelines | Worker #586 |
| 2025-12-14 | Phase 24 complete - OpenTelemetry distributed tracing with OTLP export | Worker #587 |
| 2025-12-14 | Phase 25 complete - CDN integration with presigned S3/R2 URLs | Worker #588 |
