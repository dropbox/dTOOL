//! Programming language document loaders.
//!
//! This module provides loaders for source code files in various programming languages,
//! organized by language paradigm:
//! - Systems languages (Rust, C/C++, Go, Zig, etc.)
//! - Scripting languages (Python, JavaScript, Bash, etc.)
//! - JVM languages (Java, Kotlin, Scala, Groovy)
//! - Functional languages (Haskell, OCaml, F#, Clojure, etc.)
//! - Web languages (PHP, Ruby, Perl, Lua, etc.)
//! - Shell languages (Tcsh, Csh, Ksh, Tcl, etc.)

// Systems programming languages
pub mod systems;

// Scripting languages
pub mod scripting;

// JVM languages
pub mod jvm;

// Functional languages
pub mod functional;

// Web languages
pub mod web;

// Shell languages
pub mod shell;

// Re-export all loaders from modules
pub use functional::*;
pub use jvm::*;
pub use scripting::*;
pub use shell::*;
pub use systems::*;
pub use web::*;
