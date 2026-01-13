//! NatBot: LLM-driven browser automation.
//!
//! This module implements NatBotChain, which uses an LLM to control browser
//! actions by analyzing simplified webpage representations.
//!
//! **Security Note**: This module provides code to control a web-browser.
//! The web-browser can be used to navigate to any URL (including any internal
//! network URLs) and local files. Exercise care if exposing this chain to
//! end-users.

mod chain;
mod crawler;
mod prompt;

pub use chain::NatBotChain;
pub use crawler::{Crawler, ElementInViewPort};
pub use prompt::{create_prompt, PROMPT_TEMPLATE};
