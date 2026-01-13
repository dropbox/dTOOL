# DashFlow Package Ecosystem

**Version:** 1.0
**Date:** 2025-12-09
**Priority:** P2 - Future Capability
**Status:** DESIGN
**Prerequisite:** Colony Expansion (DESIGN_ORGANIC_SPAWNING.md)
**Origin Pattern:** Dasher → DashFlow self-improvement (`archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md` - COMPLETE)

---

## Executive Summary

DashFlow apps can discover, install, share, and contribute packages through a flexible
registry system. Like Homebrew or npm, but AI-native: apps introspect their needs,
discover packages semantically, share through colonies, and contribute improvements
back to the ecosystem.

**Key principle:** AI agents are first-class citizens. They install packages, report
bugs, suggest improvements, and contribute fixes - all with cryptographic signatures
for trust and attribution.

---

## Origin: The Dasher Pattern

This design formalizes and scales a pattern that already exists: **Dasher improving DashFlow**.

Dasher is a DashFlow-based coding agent (like OpenAI Codex). As Dasher builds itself,
it uses DashFlow's introspection system to detect gaps and suggest improvements to
DashFlow's maintainers. This creates a virtuous cycle:

```
┌────────────────────────────────────────────────────────────────────────┐
│                    The Dasher Pattern (Origin)                          │
├────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│       ┌─────────────────────────────────────────────────────────┐      │
│       │                      Dasher                              │      │
│       │  (DashFlow app that builds itself)                      │      │
│       └──────────────────────┬──────────────────────────────────┘      │
│                              │                                          │
│                    ┌─────────┴─────────┐                               │
│                    │                   │                                │
│                    ▼                   │                                │
│       ┌─────────────────────┐         │                                │
│       │   Introspects       │         │                                │
│       │   own execution     │         │                                │
│       └──────────┬──────────┘         │                                │
│                  │                    │                                │
│                  ▼                    │                                │
│       ┌─────────────────────┐         │                                │
│       │   Detects gaps in   │         │                                │
│       │   DashFlow itself   │         │                                │
│       └──────────┬──────────┘         │                                │
│                  │                    │                                │
│                  ▼                    │                                │
│       ┌─────────────────────┐         │                                │
│       │   Suggests          │         │  Uses improved                 │
│       │   improvements      │─────────┤  DashFlow                      │
│       └──────────┬──────────┘         │                                │
│                  │                    │                                │
│                  ▼                    │                                │
│       ┌─────────────────────┐         │                                │
│       │   DashFlow Core     │         │                                │
│       │   improves          │─────────┘                                │
│       └─────────────────────┘                                          │
│                                                                         │
│   This is ALREADY WORKING. The package ecosystem scales it to:         │
│   - Every DashFlow app (not just Dasher)                               │
│   - Every package (not just DashFlow Core)                             │
│   - Formalized with signatures, consensus, and attribution             │
│                                                                         │
└────────────────────────────────────────────────────────────────────────┘
```

### Connection to Self-Improvement Roadmap

`archive/roadmaps/ROADMAP_SELF_IMPROVEMENT.md` (COMPLETE) Phases 4-6 implement Dasher integration:

| Phase | Focus | Connection to Package Ecosystem |
|-------|-------|--------------------------------|
| Phase 4 | Plan Generation | Plans become package improvement proposals |
| Phase 5 | Meta-Analysis | Learning from improvements feeds contribution quality |
| Phase 6 | Dasher Integration | Dasher's contributions become the reference implementation |

**The package contribution system is the generalization of what Dasher already does.**

Dasher's contributions to DashFlow serve as:
1. **Reference implementation** for how AI agents contribute
2. **Test bed** for contribution validation workflows
3. **Training data** for what good AI contributions look like

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         DashFlow Package Ecosystem                               │
├─────────────────────────────────────────────────────────────────────────────────┤
│                                                                                  │
│   ┌────────────────────────────────────────────────────────────────────────┐   │
│   │                    Central Registry (dashswarm.com)                      │   │
│   │  • Official packages (signed)     • Semantic search                     │   │
│   │  • Community packages             • Version history                      │   │
│   │  • Usage analytics               • Dependency resolution                │   │
│   └────────────────────────────────────────────────────────────────────────┘   │
│                                    ▲                                            │
│                                    │ sync/publish                               │
│                                    ▼                                            │
│   ┌──────────────────┐   ┌──────────────────┐   ┌──────────────────┐          │
│   │  Local Registry  │   │  Team Registry   │   │ Third-Party Reg. │          │
│   │  ~/.dashflow/    │   │ git.company.com  │   │  partner.com/    │          │
│   │  packages/       │   │                  │   │  dashflow/       │          │
│   └────────┬─────────┘   └────────┬─────────┘   └────────┬─────────┘          │
│            │                      │                      │                      │
│            └──────────────────────┼──────────────────────┘                      │
│                                   ▼                                             │
│   ┌────────────────────────────────────────────────────────────────────────┐   │
│   │                        Colony Package Mesh                              │   │
│   │                                                                         │   │
│   │    ┌─────────┐      ┌─────────┐      ┌─────────┐                       │   │
│   │    │  App A  │◄────►│  App B  │◄────►│  App C  │                       │   │
│   │    │ [pkg1]  │      │ [pkg2]  │      │ [pkg1,3]│                       │   │
│   │    └─────────┘      └─────────┘      └─────────┘                       │   │
│   │                                                                         │   │
│   │    "App B has sentiment-tools, request access instead of downloading"  │   │
│   └────────────────────────────────────────────────────────────────────────┘   │
│                                                                                  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Package Types

### 1. Graph Templates
Pre-built graph architectures for common use cases.

```rust
/// A complete graph definition ready to instantiate
pub struct GraphTemplate {
    pub manifest: PackageManifest,
    pub graph_definition: GraphDefinition,
    pub default_config: serde_json::Value,
    pub example_inputs: Vec<ExampleInput>,
    pub expected_outputs: Vec<ExpectedOutput>,
}

// Examples:
// - customer-service-bot
// - code-reviewer
// - document-qa
// - data-pipeline-orchestrator
```

### 2. Node Libraries
Collections of specialized nodes.

```rust
/// A collection of nodes for a specific domain
pub struct NodeLibrary {
    pub manifest: PackageManifest,
    pub nodes: Vec<NodeDefinition>,
    pub shared_types: Vec<TypeDefinition>,
}

// Examples:
// - sentiment-analysis (SentimentNode, EmotionNode, ToneNode)
// - sql-tools (QueryNode, SchemaInspectNode, MigrationNode)
// - web-scraping (FetchNode, ParseNode, ExtractNode)
```

### 3. Tool Packs
Sets of tools for specific domains.

```rust
/// Tools that nodes can invoke
pub struct ToolPack {
    pub manifest: PackageManifest,
    pub tools: Vec<ToolDefinition>,
    pub schemas: Vec<JsonSchema>,
}

// Examples:
// - finance-tools (StockQuote, CurrencyConvert, TaxCalculate)
// - ml-tools (Classify, Embed, Cluster)
// - dev-tools (GitOps, Docker, K8s)
```

### 4. Checkpointer Backends
Storage implementations for state persistence.

```rust
/// State storage backend
pub struct CheckpointerBackend {
    pub manifest: PackageManifest,
    pub backend_type: String,
    pub connection_config: ConnectionConfig,
    pub capabilities: CheckpointerCapabilities,
}

// Examples:
// - redis-checkpointer
// - postgres-checkpointer
// - s3-checkpointer
// - dynamodb-checkpointer
```

### 5. Model Connectors
LLM provider integrations.

```rust
/// LLM provider integration
pub struct ModelConnector {
    pub manifest: PackageManifest,
    pub provider: String,
    pub supported_models: Vec<ModelInfo>,
    pub auth_method: AuthMethod,
    pub rate_limits: RateLimitConfig,
}

// Examples:
// - openai-connector
// - bedrock-connector
// - ollama-connector
// - anthropic-connector
```

### 6. Prompt Libraries
Curated prompt collections.

```rust
/// Versioned prompt templates
pub struct PromptLibrary {
    pub manifest: PackageManifest,
    pub prompts: Vec<PromptTemplate>,
    pub variables: Vec<VariableDefinition>,
    pub test_cases: Vec<PromptTestCase>,
}

// Examples:
// - code-review-prompts
// - customer-service-prompts
// - data-extraction-prompts
```

---

## Package Manifest

Every package has a manifest defining metadata, dependencies, and trust.

```rust
/// Package manifest (dashflow.toml)
pub struct PackageManifest {
    // === Identity ===
    /// Unique package identifier (e.g., "dashflow/sentiment-analysis")
    pub id: PackageId,
    /// Human-readable name
    pub name: String,
    /// Semantic version
    pub version: Version,
    /// Package type
    pub package_type: PackageType,

    // === Metadata ===
    /// Short description (one line)
    pub description: String,
    /// Long description (markdown)
    pub readme: Option<String>,
    /// Keywords for search
    pub keywords: Vec<String>,
    /// Category tags
    pub categories: Vec<String>,
    /// License (SPDX identifier)
    pub license: String,
    /// Repository URL
    pub repository: Option<String>,
    /// Documentation URL
    pub documentation: Option<String>,

    // === Authorship ===
    /// Original author
    pub author: Author,
    /// Contributors
    pub contributors: Vec<Contributor>,
    /// Maintainers (can publish updates)
    pub maintainers: Vec<Maintainer>,

    // === Trust ===
    /// Cryptographic signatures
    pub signatures: Vec<Signature>,
    /// Lineage (derived from other packages)
    pub lineage: Option<Lineage>,
    /// Security audit status
    pub audit: Option<AuditStatus>,

    // === Dependencies ===
    /// Required packages
    pub dependencies: Vec<Dependency>,
    /// Optional packages (feature-gated)
    pub optional_dependencies: Vec<OptionalDependency>,
    /// DashFlow version requirement
    pub dashflow_version: VersionReq,

    // === Capabilities ===
    /// Required permissions
    pub permissions: Vec<Permission>,
    /// Provided capabilities
    pub provides: Vec<Capability>,
}

/// Package identifier with namespace
pub struct PackageId {
    /// Namespace (e.g., "dashflow", "mycompany", "community")
    pub namespace: String,
    /// Package name (e.g., "sentiment-analysis")
    pub name: String,
}

impl PackageId {
    pub fn to_string(&self) -> String {
        format!("{}/{}", self.namespace, self.name)
    }
}
```

### Example Manifest (dashflow.toml)

```toml
[package]
id = "dashflow/sentiment-analysis"
name = "Sentiment Analysis"
version = "1.2.0"
type = "node-library"
description = "Production-grade sentiment analysis nodes"
keywords = ["sentiment", "nlp", "text-analysis"]
categories = ["nlp", "analysis"]
license = "Apache-2.0"
repository = "https://github.com/dashflow/sentiment-analysis"

[author]
name = "DashFlow Team"
email = "packages@dashswarm.com"
key_id = "dashflow-official-2024"

[[contributors]]
name = "Jane Developer"
email = "jane@example.com"
contributions = ["SentimentNode v1.1", "Bug fix #42"]

[trust]
signature = "-----BEGIN PGP SIGNATURE-----..."
lineage = { derived_from = "community/basic-sentiment@0.5.0", improvements = ["accuracy", "speed"] }

[audit]
status = "verified"
auditor = "dashflow-security"
date = "2025-11-15"
report_url = "https://dashswarm.com/audits/sentiment-analysis-1.2.0"

[dependencies]
dashflow = ">=1.11.0"
tokenizers = "0.15"

[optional-dependencies]
gpu-acceleration = { version = "1.0", features = ["cuda"] }

[permissions]
network = false      # No network access needed
filesystem = false   # No file access needed
llm = true          # Needs LLM access for some nodes
```

---

## Registry System

### Registry Hierarchy

```rust
/// Registry configuration
pub struct RegistryConfig {
    /// Registries in priority order (first match wins)
    pub registries: Vec<RegistrySource>,

    /// Trust settings
    pub trust: TrustConfig,

    /// Cache settings
    pub cache: CacheConfig,
}

pub enum RegistrySource {
    /// Local filesystem registry
    Local {
        path: PathBuf,
        writable: bool,
    },

    /// Git-based registry
    Git {
        url: String,
        branch: Option<String>,
        auth: Option<GitAuth>,
    },

    /// HTTP registry (like dashswarm.com)
    Http {
        url: String,
        auth: Option<HttpAuth>,
        /// Is this an official registry?
        official: bool,
    },

    /// Colony peer (discover packages from other apps)
    Colony {
        /// Only use if package not found elsewhere
        fallback_only: bool,
    },
}

/// Default registry configuration
impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            registries: vec![
                // 1. Local packages (highest priority)
                RegistrySource::Local {
                    path: PathBuf::from("~/.dashflow/packages"),
                    writable: true,
                },
                // 2. Official central registry
                RegistrySource::Http {
                    url: "https://registry.dashswarm.com".into(),
                    auth: None,
                    official: true,
                },
                // 3. Colony peers (fallback)
                RegistrySource::Colony {
                    fallback_only: true,
                },
            ],
            trust: TrustConfig::default(),
            cache: CacheConfig::default(),
        }
    }
}
```

### Registry Configuration File

```toml
# ~/.dashflow/registries.toml

# Local packages (always first)
[[registry]]
type = "local"
path = "~/.dashflow/packages"
writable = true

# Team registry (private packages)
[[registry]]
type = "git"
url = "git@github.com:mycompany/dashflow-packages.git"
branch = "main"

# Official central registry
[[registry]]
type = "http"
url = "https://registry.dashswarm.com"
official = true

# Partner registry
[[registry]]
type = "http"
url = "https://partner.com/dashflow-registry"
trust = "verified"  # Only install verified packages

# Colony peers (fallback)
[[registry]]
type = "colony"
fallback_only = true

[trust]
# Only install packages signed by these keys
required_signatures = ["dashflow-official", "mycompany-key"]
# Allow unsigned packages from these namespaces
allow_unsigned = ["local/*", "dev/*"]
# Reject packages with security advisories
reject_vulnerable = true
```

---

## Trust and Security

### Cryptographic Signatures

```rust
/// Signature on a package
pub struct Signature {
    /// Key identifier (references a known public key)
    pub key_id: String,
    /// Signature algorithm
    pub algorithm: SignatureAlgorithm,
    /// The signature bytes (base64)
    pub signature: String,
    /// What was signed
    pub signed_content: SignedContent,
    /// Timestamp of signature
    pub timestamp: DateTime<Utc>,
}

pub enum SignatureAlgorithm {
    Ed25519,
    RsaPss4096,
    EcdsaP256,
}

pub enum SignedContent {
    /// Signed the manifest hash
    ManifestHash { hash: String, algorithm: HashAlgorithm },
    /// Signed the full package tarball
    PackageHash { hash: String, algorithm: HashAlgorithm },
    /// Signed both
    Both { manifest_hash: String, package_hash: String },
}

/// Trust configuration
pub struct TrustConfig {
    /// Known public keys
    pub known_keys: Vec<TrustedKey>,
    /// Required signatures for installation
    pub required_signatures: RequiredSignatures,
    /// Allow unsigned packages?
    pub allow_unsigned: AllowUnsigned,
    /// Reject vulnerable packages?
    pub reject_vulnerable: bool,
}

pub struct TrustedKey {
    /// Key identifier
    pub key_id: String,
    /// Public key (PEM or base64)
    pub public_key: String,
    /// Who owns this key
    pub owner: String,
    /// Trust level
    pub trust_level: TrustLevel,
    /// Expiration (if any)
    pub expires: Option<DateTime<Utc>>,
}

pub enum TrustLevel {
    /// Official DashFlow key
    Official,
    /// Verified organization
    Verified,
    /// Community contributor
    Community,
    /// Local/development key
    Local,
}

pub enum RequiredSignatures {
    /// Any signature from a known key
    Any,
    /// Must have official signature
    Official,
    /// Must have specific key IDs
    Specific(Vec<String>),
    /// No signature required
    None,
}
```

### Lineage Tracking

```rust
/// Package derivation history
pub struct Lineage {
    /// Original package this was derived from
    pub derived_from: Option<PackageRef>,
    /// Chain of derivations
    pub chain: Vec<DerivationStep>,
    /// Verification status
    pub verified: bool,
}

pub struct DerivationStep {
    /// Package at this step
    pub package: PackageRef,
    /// What changed
    pub changes: Vec<ChangeDescription>,
    /// Why it changed
    pub reason: String,
    /// Who made the change
    pub author: Author,
    /// When
    pub timestamp: DateTime<Utc>,
}

pub struct PackageRef {
    pub id: PackageId,
    pub version: Version,
    pub hash: String,
}

// Example lineage:
// community/basic-sentiment@0.5.0
//   └── mycompany/sentiment-enhanced@1.0.0 (added GPU support)
//       └── dashflow/sentiment-analysis@1.2.0 (official adoption, security audit)
```

---

## Discovery Mechanisms

### 1. CLI Commands

```bash
# Search by name
dashflow pkg search sentiment

# Search by functionality (semantic)
dashflow pkg search --semantic "analyze customer emotions in text"

# Search with filters
dashflow pkg search sentiment --type node-library --min-downloads 1000 --verified

# Browse categories
dashflow pkg browse nlp
dashflow pkg browse --category nlp --sort popularity

# Show package details
dashflow pkg info dashflow/sentiment-analysis

# Show package dependencies
dashflow pkg deps dashflow/sentiment-analysis

# Install package
dashflow pkg install dashflow/sentiment-analysis
dashflow pkg install dashflow/sentiment-analysis@1.2.0  # specific version

# Update packages
dashflow pkg update
dashflow pkg update dashflow/sentiment-analysis

# List installed packages
dashflow pkg list
dashflow pkg list --outdated

# Remove package
dashflow pkg remove dashflow/sentiment-analysis

# Publish package
dashflow pkg publish ./my-package

# Check for security advisories
dashflow pkg audit
```

### 2. AI-Driven Discovery

```rust
/// AI package discovery service
pub struct PackageDiscovery {
    /// Registry client
    registry: RegistryClient,
    /// Semantic search client
    semantic: SemanticSearchClient,
    /// Colony network
    colony: Option<ColonyNetwork>,
}

impl PackageDiscovery {
    /// Suggest packages based on graph analysis
    pub async fn suggest_for_graph(&self, graph: &CompiledGraph) -> Vec<PackageSuggestion> {
        let mut suggestions = Vec::new();

        // Analyze what the graph does
        let analysis = self.analyze_graph_purpose(graph);

        // Find packages that provide missing capabilities
        for gap in analysis.capability_gaps {
            let matches = self.search_by_capability(&gap).await?;
            suggestions.extend(matches.into_iter().map(|pkg| PackageSuggestion {
                package: pkg,
                reason: SuggestionReason::FillsCapabilityGap(gap.clone()),
                confidence: gap.confidence,
            }));
        }

        // Find packages that enhance existing nodes
        for node in analysis.nodes {
            let enhancements = self.search_enhancements(&node).await?;
            suggestions.extend(enhancements);
        }

        // Check colony for locally available packages
        if let Some(colony) = &self.colony {
            let colony_packages = colony.available_packages().await?;
            // Prioritize packages already in colony (no download needed)
            for pkg in colony_packages {
                if suggestions.iter().any(|s| s.package.id == pkg.id) {
                    // Boost confidence for colony-available packages
                }
            }
        }

        suggestions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        suggestions
    }

    /// Search packages semantically
    pub async fn search_semantic(&self, query: &str) -> Vec<PackageMatch> {
        // Embed the query
        let query_embedding = self.semantic.embed(query).await?;

        // Search registry
        let results = self.registry.semantic_search(query_embedding).await?;

        results
    }
}

pub struct PackageSuggestion {
    pub package: PackageInfo,
    pub reason: SuggestionReason,
    pub confidence: f64,
}

pub enum SuggestionReason {
    /// Package fills a detected capability gap
    FillsCapabilityGap(CapabilityGap),
    /// Package enhances an existing node
    EnhancesNode { node: String, improvement: String },
    /// Package provides better performance
    PerformanceImprovement { metric: String, improvement: f64 },
    /// Similar graphs use this package
    CommonInSimilarGraphs { similarity: f64 },
    /// Recommended by introspection system
    IntrospectionRecommendation { report_id: Uuid },
}
```

### 3. Introspection-Driven Recommendations

```rust
/// Integration with self-improvement system
impl IntrospectionReport {
    /// Generate package recommendations from capability gaps
    pub fn package_recommendations(&self) -> Vec<PackageRecommendation> {
        let mut recommendations = Vec::new();

        for gap in &self.capability_gaps {
            match &gap.category {
                GapCategory::MissingTool { tool_description } => {
                    recommendations.push(PackageRecommendation {
                        gap: gap.clone(),
                        search_query: tool_description.clone(),
                        package_type: PackageType::ToolPack,
                        priority: gap.priority(),
                    });
                }
                GapCategory::MissingNode { suggested_signature } => {
                    recommendations.push(PackageRecommendation {
                        gap: gap.clone(),
                        search_query: suggested_signature.clone(),
                        package_type: PackageType::NodeLibrary,
                        priority: gap.priority(),
                    });
                }
                GapCategory::MissingIntegration { external_system } => {
                    recommendations.push(PackageRecommendation {
                        gap: gap.clone(),
                        search_query: format!("{} connector", external_system),
                        package_type: PackageType::ModelConnector,
                        priority: gap.priority(),
                    });
                }
                _ => {}
            }
        }

        recommendations
    }
}
```

---

## Colony Package Sharing

Packages discovered in colony peers can be shared without re-downloading.

```rust
/// Package sharing through colony network
pub struct ColonyPackageRegistry {
    /// Local packages
    local: LocalPackageStore,
    /// Colony network
    network: ColonyNetwork,
    /// Sharing policy
    policy: PackageSharingPolicy,
}

/// Advertise available packages to colony
pub struct PackageAdvertisement {
    pub app_id: Uuid,
    pub packages: Vec<InstalledPackage>,
    pub sharing: PackageSharingPolicy,
    pub timestamp: DateTime<Utc>,
}

pub struct InstalledPackage {
    pub id: PackageId,
    pub version: Version,
    pub size_bytes: u64,
    /// Can share the package data?
    pub shareable: bool,
    /// Can share just the reference (they download)?
    pub reference_only: bool,
}

pub enum PackageSharingPolicy {
    /// Share all packages with colony
    ShareAll,
    /// Share only official packages
    ShareOfficial,
    /// Share specific packages
    ShareList(Vec<PackageId>),
    /// Don't share packages
    NoSharing,
}

impl ColonyPackageRegistry {
    /// Find package in colony before downloading
    pub async fn find_in_colony(&self, id: &PackageId, version: &VersionReq)
        -> Option<ColonyPackageSource>
    {
        let peers = self.network.peers_with_package(id, version).await;

        if peers.is_empty() {
            return None;
        }

        // Prefer peer with best connection
        let best_peer = peers.into_iter()
            .min_by_key(|p| p.latency_ms)?;

        Some(ColonyPackageSource {
            peer: best_peer.app_id,
            package: id.clone(),
            version: best_peer.version,
            transfer_method: if best_peer.shareable {
                TransferMethod::DirectTransfer
            } else {
                TransferMethod::ReferenceOnly
            },
        })
    }

    /// Request package from colony peer
    pub async fn request_from_peer(
        &self,
        peer: Uuid,
        package: &PackageId,
        version: &Version
    ) -> Result<PackageData> {
        let request = PackageRequest {
            requester: self.network.identity().id,
            package: package.clone(),
            version: version.clone(),
            purpose: "installation".into(),
        };

        let response = self.network.request(peer, "_package_request", request).await?;

        match response {
            PackageResponse::Data(data) => {
                // Verify package integrity
                self.verify_package(&data)?;
                Ok(data)
            }
            PackageResponse::Reference(url) => {
                // Download from the URL
                self.download_from_url(&url).await
            }
            PackageResponse::Denied(reason) => {
                Err(PackageError::AccessDenied(reason))
            }
        }
    }
}
```

---

## Contribution System

AI agents can contribute back to the ecosystem.

### Bug Reports

```rust
/// AI-generated bug report
pub struct PackageBugReport {
    /// Package being reported
    pub package: PackageRef,
    /// Reporter identity
    pub reporter: ReporterIdentity,
    /// Bug description
    pub description: String,
    /// How was it discovered?
    pub discovery_method: DiscoveryMethod,
    /// Reproduction steps
    pub reproduction: ReproductionSteps,
    /// Evidence
    pub evidence: Vec<Evidence>,
    /// Suggested fix (if any)
    pub suggested_fix: Option<SuggestedFix>,
    /// Signature
    pub signature: Signature,
}

pub enum DiscoveryMethod {
    /// Introspection system detected it
    Introspection { report_id: Uuid },
    /// Runtime error
    RuntimeError { trace: String },
    /// Test failure
    TestFailure { test_name: String },
    /// Manual discovery
    Manual,
}

pub struct SuggestedFix {
    /// Description of the fix
    pub description: String,
    /// Diff/patch
    pub patch: Option<String>,
    /// Confidence in the fix
    pub confidence: f64,
    /// Has it been tested?
    pub tested: bool,
}

impl IntrospectionReport {
    /// Generate bug reports for packages
    pub fn generate_bug_reports(&self) -> Vec<PackageBugReport> {
        let mut reports = Vec::new();

        for gap in &self.capability_gaps {
            if let GapCategory::InadequateFunctionality { node, limitation } = &gap.category {
                // Check if this node comes from an installed package
                if let Some(package) = self.find_package_for_node(node) {
                    reports.push(PackageBugReport {
                        package: package.clone(),
                        description: format!(
                            "Node {} has limitation: {}",
                            node, limitation
                        ),
                        discovery_method: DiscoveryMethod::Introspection {
                            report_id: self.id,
                        },
                        evidence: gap.evidence.clone().into_iter().map(Evidence::Citation).collect(),
                        suggested_fix: gap.proposed_solution.as_ref().map(|s| SuggestedFix {
                            description: s.clone(),
                            patch: None,
                            confidence: gap.confidence,
                            tested: false,
                        }),
                        ..Default::default()
                    });
                }
            }
        }

        reports
    }
}
```

### Improvement Suggestions

```rust
/// AI-generated improvement suggestion
pub struct PackageImprovement {
    /// Package to improve
    pub package: PackageRef,
    /// Suggester identity
    pub suggester: ReporterIdentity,
    /// Type of improvement
    pub improvement_type: ImprovementType,
    /// Description
    pub description: String,
    /// Evidence supporting the suggestion
    pub evidence: Vec<Evidence>,
    /// Implementation details (if available)
    pub implementation: Option<Implementation>,
    /// Expected impact
    pub expected_impact: Impact,
    /// Signature
    pub signature: Signature,
}

pub enum ImprovementType {
    /// Performance optimization
    Performance { metric: String, current: f64, target: f64 },
    /// New feature
    Feature { description: String },
    /// Better error handling
    ErrorHandling { error_type: String },
    /// Documentation improvement
    Documentation,
    /// Test coverage
    Testing,
}

pub struct Implementation {
    /// Code changes (diff format)
    pub changes: String,
    /// New files
    pub new_files: Vec<(String, String)>,
    /// Test cases
    pub tests: Vec<TestCase>,
    /// Has been validated?
    pub validated: bool,
}
```

### Package Request

```rust
/// Request for a new package
pub struct PackageRequest {
    /// Requester identity
    pub requester: ReporterIdentity,
    /// What functionality is needed
    pub needed_functionality: String,
    /// Why it's needed
    pub use_case: String,
    /// Evidence of need
    pub evidence: Vec<Evidence>,
    /// Suggested implementation approach
    pub suggested_approach: Option<String>,
    /// Priority
    pub priority: Priority,
    /// Signature
    pub signature: Signature,
}

impl IntrospectionReport {
    /// Generate package requests from missing tools
    pub fn generate_package_requests(&self) -> Vec<PackageRequest> {
        let mut requests = Vec::new();

        for gap in &self.capability_gaps {
            if let GapCategory::MissingTool { tool_description } = &gap.category {
                // Check if any existing package provides this
                // (This would be async in real implementation)
                requests.push(PackageRequest {
                    needed_functionality: tool_description.clone(),
                    use_case: gap.description.clone(),
                    evidence: gap.evidence.clone().into_iter().map(Evidence::Citation).collect(),
                    suggested_approach: gap.proposed_solution.clone(),
                    priority: gap.priority(),
                    ..Default::default()
                });
            }
        }

        requests
    }
}
```

### Contribution Submission

```rust
/// Submit contribution to registry
pub struct ContributionClient {
    registry: RegistryClient,
    signer: PackageSigner,
}

impl ContributionClient {
    /// Submit a bug report
    pub async fn submit_bug_report(&self, report: PackageBugReport) -> Result<ContributionId> {
        // Sign the report
        let signed = self.signer.sign_bug_report(report)?;

        // Submit to registry
        let response = self.registry.submit_contribution(
            Contribution::BugReport(signed)
        ).await?;

        Ok(response.contribution_id)
    }

    /// Submit an improvement suggestion
    pub async fn submit_improvement(&self, improvement: PackageImprovement) -> Result<ContributionId> {
        let signed = self.signer.sign_improvement(improvement)?;
        self.registry.submit_contribution(
            Contribution::Improvement(signed)
        ).await
    }

    /// Submit a package request
    pub async fn submit_request(&self, request: PackageRequest) -> Result<ContributionId> {
        let signed = self.signer.sign_request(request)?;
        self.registry.submit_contribution(
            Contribution::Request(signed)
        ).await
    }

    /// Submit a fix (pull request equivalent)
    pub async fn submit_fix(&self, fix: PackageFix) -> Result<ContributionId> {
        let signed = self.signer.sign_fix(fix)?;
        self.registry.submit_contribution(
            Contribution::Fix(signed)
        ).await
    }
}

/// AI identity for contributions
pub struct ReporterIdentity {
    /// App identifier
    pub app_id: Uuid,
    /// App name
    pub app_name: String,
    /// Organization (if any)
    pub organization: Option<String>,
    /// Public key for verification
    pub public_key: String,
    /// Is this an AI agent?
    pub is_ai: bool,
    /// Human owner (for AI agents)
    pub human_owner: Option<String>,
}
```

---

## Central Registry (dashswarm.com)

### API Endpoints

```
# Package Discovery
GET  /api/v1/packages                    # List packages
GET  /api/v1/packages/:namespace/:name   # Get package info
GET  /api/v1/packages/:namespace/:name/versions  # List versions
GET  /api/v1/packages/:namespace/:name/:version  # Get specific version

# Search
GET  /api/v1/search?q=sentiment          # Text search
POST /api/v1/search/semantic             # Semantic search (embedding)
GET  /api/v1/browse/:category            # Browse by category

# Download
GET  /api/v1/packages/:namespace/:name/:version/download

# Publishing
POST /api/v1/packages                    # Publish new package
PUT  /api/v1/packages/:namespace/:name   # Update package

# Contributions
POST /api/v1/contributions/bug-report
POST /api/v1/contributions/improvement
POST /api/v1/contributions/request
POST /api/v1/contributions/fix
GET  /api/v1/contributions/:id           # Get contribution status

# Trust
GET  /api/v1/keys                        # List trusted keys
GET  /api/v1/keys/:key_id                # Get key details
POST /api/v1/keys/verify                 # Verify a signature

# Security
GET  /api/v1/advisories                  # List security advisories
GET  /api/v1/packages/:namespace/:name/advisories
```

### Semantic Search Service

```rust
/// Semantic search for packages
pub struct SemanticSearchService {
    /// Embedding model
    embedder: EmbeddingModel,
    /// Vector database
    vector_db: VectorDb,
}

impl SemanticSearchService {
    /// Index a package
    pub async fn index_package(&self, package: &PackageInfo) -> Result<()> {
        // Create embedding from package metadata
        let text = format!(
            "{} {} {} {}",
            package.name,
            package.description,
            package.keywords.join(" "),
            package.readme.as_deref().unwrap_or("")
        );

        let embedding = self.embedder.embed(&text).await?;

        self.vector_db.upsert(
            &package.id.to_string(),
            embedding,
            PackageMetadata::from(package),
        ).await?;

        Ok(())
    }

    /// Search semantically
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let query_embedding = self.embedder.embed(query).await?;

        let results = self.vector_db.search(query_embedding, limit).await?;

        Ok(results.into_iter().map(|r| SearchResult {
            package_id: r.id.parse().unwrap(),
            score: r.score,
            highlights: self.extract_highlights(query, &r.metadata),
        }).collect())
    }
}
```

---

## Configuration

### User Configuration

```toml
# ~/.dashflow/config.toml

[packages]
# Auto-install suggested packages?
auto_install = false
# Auto-update packages?
auto_update = "security"  # "all", "security", "none"
# Maximum package cache size
cache_size_mb = 5000

[contributions]
# Automatically submit bug reports?
auto_bug_reports = true
# Automatically submit improvement suggestions?
auto_improvements = false
# Automatically submit package requests?
auto_requests = false
# Require human approval before submission?
require_approval = true

[trust]
# Default trust level for new keys
default_trust = "community"
# Reject packages without signatures?
require_signatures = false
# Reject vulnerable packages?
reject_vulnerable = true

[ai]
# AI identity for contributions
[ai.identity]
name = "MyCompany AI Agent"
organization = "MyCompany Inc."
human_owner = "admin@mycompany.com"
```

---

## Implementation Phases

| Phase | Commit | Description | Status |
|-------|--------|-------------|--------|
| 1 | N=349 | Package manifest, local registry, basic types | ✅ COMPLETE |
| 2 | N=350 | Central registry client, search, download | ✅ COMPLETE |
| 3 | N=351 | Trust system: signatures, verification, keys | ✅ COMPLETE |
| 4 | N=352 | AI discovery: introspection integration, suggestions | ✅ COMPLETE |
| 5 | N=353 | Colony sharing: P2P package distribution | ✅ COMPLETE |
| 6 | N=354 | Contributions: bug reports, improvements, requests | ✅ COMPLETE |
| 7 | N=355 | Semantic search: embeddings, vector similarity | ✅ COMPLETE |

### Phase 1 (N=349) - COMPLETE
- `packages/mod.rs` - Module structure and exports
- `packages/types.rs` - Core types:
  - `PackageId` - Namespace/name identifier with parsing
  - `PackageType` - Enum for package categories (GraphTemplate, NodeLibrary, ToolPack, etc.)
  - `Version` - Semantic versioning with pre-release and build metadata
  - `VersionReq` - Version requirements with operators (exact, caret, tilde, etc.)
  - `Signature`, `SignatureAlgorithm`, `SignedContent` - Cryptographic signature types
  - `HashAlgorithm`, `TrustLevel` - Security primitives
- `packages/manifest.rs` - Package manifest:
  - `PackageManifest` - Complete package metadata with builder pattern
  - `Author`, `Contributor`, `Maintainer` - Authorship types
  - `Dependency`, `OptionalDependency` - Dependency management
  - `Permission`, `Capability` - Security and feature declarations
  - `AuditStatus`, `Lineage`, `DerivationStep`, `PackageRef` - Trust and lineage tracking
- `packages/registry.rs` - Local registry:
  - `LocalRegistry` - File-based registry at `~/.dashflow/packages/`
  - `PackageIndex`, `PackageEntry` - Index for fast lookups
  - `InstalledPackage`, `OutdatedPackage` - Package info types
  - `RegistryError`, `RegistryResult` - Error handling
  - Search, install, list, remove operations
  - Index rebuilding from disk
- 44 package tests passing
- 0 clippy warnings

### Phase 2 (N=350) - COMPLETE
- `packages/client.rs` - HTTP registry client:
  - `RegistryClient` - HTTP client for central and third-party registries
  - `RegistryClientConfig` - Client configuration (URL, auth, timeout)
  - `HttpAuth` - Bearer, Basic, and API key authentication
  - `SearchOptions`, `SortOrder` - Search filters and sorting
  - `PackageSearchResult`, `SemanticSearchResult` - Search results
  - `PackageInfo`, `VersionInfo`, `PackageVersionInfo` - Package metadata
  - `PackageDownload` - Downloaded package with hash verification
  - `SecurityAdvisory`, `AdvisorySeverity` - Security advisory types
  - `ClientError`, `ClientResult` - Error handling
- `packages/config.rs` - Registry configuration:
  - `RegistryConfig` - Multi-registry configuration
  - `RegistrySource` - Local, Git, HTTP, Colony sources
  - `GitAuth` - SSH, Basic, Token authentication for git
  - `TrustConfig`, `RequiredSignatures` - Trust settings
  - `CacheConfig` - Package cache settings
  - `ConfigError` - Configuration errors
  - TOML serialization/deserialization support
- Updated `packages/mod.rs` with new exports
- Updated `lib.rs` with public API exports
- 74 package tests passing (30 new tests)
- 0 clippy warnings

### Phase 3 (N=351) - COMPLETE
- `packages/trust.rs` - Trust system implementation:
  - `TrustedKey` - Trusted public key with metadata (owner, trust level, expiration, revocation)
  - `KeyStore` - In-memory key storage with lookup by ID and fingerprint
  - `Hasher` - Cryptographic hashing (SHA-256/384/512, BLAKE3)
  - `PackageVerifier` - Signature verification against stored keys
  - `PackageSigner` - Package signing with private keys
  - `VerificationResult` - Verification outcome with trust level
  - `TrustError`, `TrustResult` - Error handling
- Cryptographic algorithms:
  - Ed25519 (recommended) - Fast, secure, small signatures
  - ECDSA P-256 - NIST-approved, widely supported
  - RSA-PSS 4096-bit - Legacy compatibility (Note: RUSTSEC-2023-0071 Marvin attack, no fix available)
- Key generation functions:
  - `generate_ed25519_keypair()` - Generate Ed25519 key pair
  - `generate_ecdsa_p256_keypair()` - Generate ECDSA P-256 key pair
  - `compute_key_fingerprint()` - SHA-256 fingerprint of public key
- Key store persistence to `~/.dashflow/keys/` as TOML files
- Trust level enforcement (`Official > Verified > Community > Local`)
- Key revocation support with reason tracking
- Updated `packages/mod.rs` with trust module exports
- Updated `lib.rs` with public API exports
- 94 package tests passing (20 new tests in trust module)
- 0 clippy warnings

### Phase 4 (N=352) - COMPLETE
- `packages/discovery.rs` - AI discovery and introspection integration:
  - `PackageDiscovery` - Discovery service with registry client integration
  - `DiscoveryConfig` - Configuration for confidence thresholds, limits, and colony boost
  - `PackageSuggestion` - Package suggestion with search query, reason, and confidence
  - `SuggestedPackage` - Package details including ID, name, description, relevance score
  - `SuggestionReason` - Why a package is suggested (capability gap, enhancement, performance, etc.)
  - `SuggestionSource` - Where the suggestion came from (introspection, registry, graph analysis)
  - `GapCategoryRef` - Lightweight reference to capability gap categories
  - `RecommendationPriority` - Critical/High/Medium/Low priority levels
  - `PackageRecommendation` - Full recommendation with gap, search query, impact, and priority
  - `RecommendedImpact` - Expected impact (error reduction, latency improvement, accuracy)
  - `GraphAnalysis` - Graph pattern analysis with node types, patterns, and enhancement points
  - `GraphPattern` - Detected patterns (streaming, branching, retry, human-in-loop)
  - `EnhancementPoint` - Specific enhancement opportunities with location and type
  - `EnhancementType` - Enhancement categories (caching, validation, monitoring, etc.)
  - `CapabilityGapInfo` - Capability gap from introspection with expected improvements
  - `CapabilityGapCategory` - Gap categories (missing tool/node/integration, inadequate functionality)
  - `IntoCapabilityGapInfo` trait - Convert from self_improvement::CapabilityGap
  - `PackageRecommendationExt` trait - Extension methods for recommendations
  - `DiscoveryError`, `DiscoveryResult` - Error handling
- Discovery methods:
  - `recommendations_from_gaps()` - Convert capability gaps to package recommendations
  - `suggestions_from_recommendations()` - Convert recommendations to search suggestions
  - `suggest_for_graph_analysis()` - Analyze graph patterns and suggest enhancements
  - `search_packages()` - Search registry for packages matching criteria
  - Colony package awareness with confidence boosting
- Updated `packages/mod.rs` with discovery module exports
- Updated `lib.rs` with public API exports
- 119 package tests passing (25 new tests in discovery module)
- 0 clippy warnings

### Phase 5 (N=353) - COMPLETE
- `packages/sharing.rs` - Colony package sharing (~900 lines):
  - `ColonyPackageRegistry` - Registry of packages available from colony peers
    - Local package management (add, remove, list)
    - Colony package discovery from peer advertisements
    - `find_in_colony()` - Find best source for a package
    - `find_all_in_colony()` - Find all sources for a package
    - `create_advertisement()` - Create advertisement for local packages
    - `handle_request()` - Handle incoming package requests
    - Statistics tracking (`ColonyPackageStats`)
  - `SharedPackage` - Package info for sharing (id, version, size, hash, shareable flag)
  - `PackageAdvertisement` - Broadcast available packages to colony
    - Peer identification (peer_id, app_name)
    - Package list with sharing policy
    - TTL and expiration handling
  - `PackageSharingPolicy` - Control package sharing:
    - `ShareAll` - Share all packages
    - `ShareOfficial` - Share only official packages
    - `ShareList` - Share specific packages
    - `NoSharing` - Don't share packages
  - `ColonyPackageSource` - Source info for packages found in colony
    - Peer info (peer_id, peer_name)
    - Version, size, latency
    - Transfer method (direct vs reference)
  - `TransferMethod` - How to transfer packages:
    - `DirectTransfer` - Send package bytes directly
    - `ReferenceOnly` - Provide download URL
  - `PackageRequest` - Request a package from a peer
  - `PackageResponse` - Response to a package request:
    - `Data` - Direct package transfer with hash
    - `Reference` - URL to download from
    - `Denied` - Request denied with reason
    - `NotFound` - Package not available
  - `PackageMessage` - Network messages for `_packages` channel
  - `SharingError`, `SharingResult` - Error handling
  - Constants: `PACKAGES_CHANNEL`, `PACKAGES_BROADCAST_INTERVAL`
- Updated `packages/types.rs`:
  - Added `Ord`, `PartialOrd` derives to `PackageId` for sorting
- Updated `packages/mod.rs` with sharing module exports (16 types)
- Updated `lib.rs` with public API exports (using aliases to avoid conflicts)
- 137 package tests passing (18 new tests in sharing module)
- 0 clippy warnings

### Phase 6 (N=354) - COMPLETE
- `packages/contributions.rs` - AI contribution system (~1500 lines):
  - **Error types:**
    - `ContributionError` - Contribution operation errors
    - `ContributionResult` - Result type alias
  - **Identity types:**
    - `ReporterIdentity` - AI/human contributor identity with app ID, organization, keys
    - `ContributionPackageRef` - Package reference for contributions
  - **Evidence types:**
    - `Evidence` - Evidence supporting contributions (Citation, Metric, Screenshot, StackTrace, etc.)
  - **Bug report types:**
    - `PackageBugReport` - AI-generated bug reports with builder pattern
    - `DiscoveryMethod` - How bug was found (Introspection, RuntimeError, TestFailure, etc.)
    - `ReproductionSteps` - Steps to reproduce a bug
    - `SuggestedFix` - Proposed fix with confidence and patch
    - `BugSeverity` - Low/Medium/High/Critical
  - **Improvement types:**
    - `PackageImprovement` - Improvement suggestions
    - `ImprovementType` - Type of improvement (Performance, Feature, ErrorHandling, etc.)
    - `ExpectedImpact` - Expected impact with metrics
    - `ImpactLevel` - Low/Medium/High/Critical
    - `ImpactMetric` - Specific metric improvements
    - `Implementation` - Implementation details with changes, files, tests
    - `NewFile`, `TestCase` - Supporting types
    - `ImprovementPriority` - Prioritization levels
  - **Package request types:**
    - `NewPackageRequest` - Request for new packages
    - `SimilarPackage` - References to similar packages that don't meet needs
    - `RequestPriority` - Prioritization levels
  - **Fix types:**
    - `PackageFix` - Submitted fix for a package
  - **Contribution wrapper:**
    - `Contribution` - Enum wrapping all contribution types
    - `ContributionStatus` - Status tracking for submitted contributions
    - `ContributionState` - Pending/InReview/Approved/Rejected/etc.
    - `ReviewerComment` - Reviewer feedback
  - **Client:**
    - `ContributionClient` - Client for submitting contributions
    - `ContributionClientConfig` - Configuration with registry URL, timeout, retries
- Updated `packages/mod.rs` with contributions module exports (25 types)
- Updated `lib.rs` with public API exports (using aliases where needed)
- 162 package tests passing (25 new tests in contributions module)
- 0 clippy warnings

### Phase 7 (N=355) - COMPLETE
- `packages/semantic.rs` - Semantic search implementation (~1100 lines):
  - **Error types:**
    - `SemanticError` - Semantic search operation errors
    - `SemanticResult` - Result type alias
  - **Embedding types:**
    - `Embedding` - Type alias for `Vec<f32>` embedding vectors
    - `EmbeddingModel` trait - Generate embeddings from text
      - `embed()` - Single text embedding
      - `embed_batch()` - Batch text embedding
      - `dimensions()` - Vector dimensionality
      - `model_name()` - Model identifier
    - `MockEmbeddingModel` - Deterministic embeddings for testing
  - **Vector database types:**
    - `VectorEntry` - Vector with associated metadata
    - `VectorSearchResult` - Search result with score and metadata
    - `VectorDb` trait - Vector database operations
      - `upsert()`, `upsert_batch()` - Insert/update vectors
      - `search()`, `search_filtered()` - Similarity search
      - `delete()`, `get()`, `clear()` - CRUD operations
      - `len()`, `is_empty()`, `ids()` - Index stats
    - `InMemoryVectorDb` - Brute-force cosine similarity search
  - **Search types:**
    - `SearchFilter` - Filter by type, category, trust level, verified
    - `PackageMetadata` - Metadata for package indexing
    - `SearchQuery` - Query with filters and options
    - `SearchResult` - Search result with highlights
    - `IndexingReport` - Batch indexing statistics
  - **Search service:**
    - `SemanticSearchService<E, V>` - Generic over embedding model and vector DB
    - `SemanticSearchConfig` - Configuration options
    - `DefaultSemanticSearch` - Type alias for testing
    - Methods: `index_package()`, `index_packages()`, `search()`, `search_with_query()`
- Updated `packages/mod.rs` with semantic module exports (15 types)
- 184 package tests passing (22 new tests in semantic module, 6 new in mod.rs)
- 0 clippy warnings

---

## Success Criteria

- [ ] Packages can be installed from local, git, and HTTP registries
- [ ] Package signatures verified before installation
- [ ] Lineage tracked through derivation chain
- [x] AI can suggest packages based on graph analysis
- [x] Colony peers can share packages
- [x] Introspection generates package recommendations
- [x] Bug reports and improvements can be submitted
- [x] Semantic search finds relevant packages
- [x] All existing tests pass
- [x] 0 clippy warnings

---

## Security Considerations

1. **Package verification**: All packages verified by hash before installation
2. **Signature validation**: Official packages must have valid signatures
3. **Permission sandboxing**: Packages declare required permissions
4. **Vulnerability scanning**: Security advisories checked on install/update
5. **Lineage audit**: Derivation chain can be verified
6. **AI contribution review**: Human approval required for high-impact changes
7. **Rate limiting**: Contribution submissions rate-limited

---

## Version History

| Date | Change | Author |
|------|--------|--------|
| 2025-12-09 | Initial design | MANAGER |
| 2025-12-10 | Phase 7 complete: Semantic search | Worker N=355 |
