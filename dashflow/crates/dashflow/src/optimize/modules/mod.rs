// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name modules
//! @category optimize
//! @status stable
//!
//! # DashOptimize Modules
//!
//! Pre-built node types with specific prompting patterns:
//! - **Avatar**: Advanced agent pattern with explicit action tracking and instruction optimization
//! - **BestOfN**: Output refinement through N-times sampling and reward-based selection
//! - **ChainOfThought**: Adds step-by-step reasoning before answers
//! - **Ensemble**: Parallel execution of multiple nodes with result aggregation
//! - **Refine**: Iterative refinement with feedback-driven improvement
//! - **ReAct**: Agent loop with tool use
//! - **MultiChainComparison**: Compares multiple reasoning attempts and synthesizes final answer
//!
//! These modules can be optimized using BootstrapFewShot and other optimizers.

pub mod avatar;
pub mod best_of_n;
pub mod chain_of_thought;
pub mod ensemble;
pub mod multi_chain_comparison;
pub mod react;
pub mod refine;

pub use avatar::{Action, ActionOutput, AvatarNode, AvatarTool};
pub use best_of_n::{BestOfNNode, RewardFn};
pub use chain_of_thought::ChainOfThoughtNode;
pub use ensemble::{AggregationStrategy, EnsembleNode};
pub use multi_chain_comparison::MultiChainComparisonNode;
pub use react::{ReActNode, SimpleTool, Tool};
pub use refine::{FeedbackFn, RefineNode, RefineableState};
