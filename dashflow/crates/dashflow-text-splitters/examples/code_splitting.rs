//! Code Splitting Example
//!
//! This example demonstrates how to use RecursiveCharacterTextSplitter
//! with language-specific separators to split code along natural boundaries
//! like function definitions, class definitions, and control flow statements.
//!
//! This is useful for:
//! - Building RAG systems over codebases
//! - Code search and analysis
//! - Generating code documentation
//! - Training code models

use dashflow_text_splitters::{Language, RecursiveCharacterTextSplitter, TextSplitter};

fn main() {
    println!("=== Code Splitting Example ===\n");

    // Example 1: Python code splitting
    println!("1. Python Code Splitting");
    println!("{}", "-".repeat(50));

    let python_code = r#"
class DataProcessor:
    """A class for processing data."""

    def __init__(self, name):
        self.name = name
        self.data = []

    def add_data(self, item):
        """Add an item to the data list."""
        self.data.append(item)
        return len(self.data)

    def process(self):
        """Process all data items."""
        results = []
        for item in self.data:
            if item > 0:
                results.append(item * 2)
        return results

def standalone_function(x, y):
    """A standalone utility function."""
    return x + y

def another_function():
    """Another function."""
    print("Hello from another function")
"#;

    let python_splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
        .with_chunk_size(200)
        .with_chunk_overlap(20);

    let python_chunks = python_splitter.split_text(python_code);

    println!("Split Python code into {} chunks:\n", python_chunks.len());
    for (i, chunk) in python_chunks.iter().enumerate() {
        println!("Chunk {}:", i + 1);
        println!("{}", chunk);
        println!("({} characters)\n", chunk.len());
    }

    // Example 2: Rust code splitting
    println!("\n2. Rust Code Splitting");
    println!("{}", "-".repeat(50));

    let rust_code = r#"
/// A simple calculator module
pub mod calculator {
    /// Add two numbers
    pub fn add(a: i32, b: i32) -> i32 {
        a + b
    }

    /// Subtract two numbers
    pub fn subtract(a: i32, b: i32) -> i32 {
        a - b
    }

    /// Multiply two numbers
    pub fn multiply(a: i32, b: i32) -> i32 {
        a * b
    }
}

fn main() {
    let result = calculator::add(5, 3);
    println!("5 + 3 = {}", result);

    if result > 0 {
        println!("Result is positive");
    }

    for i in 0..result {
        println!("Count: {}", i);
    }
}

const MAX_VALUE: i32 = 100;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(calculator::add(2, 2), 4);
    }
}
"#;

    let rust_splitter = RecursiveCharacterTextSplitter::from_language(Language::Rust)
        .with_chunk_size(250)
        .with_chunk_overlap(20);

    let rust_chunks = rust_splitter.split_text(rust_code);

    println!("Split Rust code into {} chunks:\n", rust_chunks.len());
    for (i, chunk) in rust_chunks.iter().enumerate() {
        println!("Chunk {}:", i + 1);
        println!("{}", chunk);
        println!("({} characters)\n", chunk.len());
    }

    // Example 3: JavaScript/TypeScript code splitting
    println!("\n3. JavaScript/TypeScript Code Splitting");
    println!("{}", "-".repeat(50));

    let js_code = r#"
class UserManager {
    constructor() {
        this.users = [];
    }

    addUser(user) {
        this.users.push(user);
        return this.users.length;
    }

    findUser(id) {
        return this.users.find(u => u.id === id);
    }
}

function processData(data) {
    const results = [];

    for (const item of data) {
        if (item.valid) {
            results.push(item.value * 2);
        }
    }

    return results;
}

const API_ENDPOINT = "https://api.example.com";

let globalCounter = 0;

async function fetchUserData(userId) {
    const response = await fetch(`${API_ENDPOINT}/users/${userId}`);
    const data = await response.json();
    return data;
}
"#;

    let js_splitter = RecursiveCharacterTextSplitter::from_language(Language::Js)
        .with_chunk_size(200)
        .with_chunk_overlap(20);

    let js_chunks = js_splitter.split_text(js_code);

    println!("Split JavaScript code into {} chunks:\n", js_chunks.len());
    for (i, chunk) in js_chunks.iter().enumerate() {
        println!("Chunk {}:", i + 1);
        println!("{}", chunk);
        println!("({} characters)\n", chunk.len());
    }

    // Example 4: Comparing languages
    println!("\n4. Language Separator Comparison");
    println!("{}", "-".repeat(50));

    let languages = vec![
        (Language::Python, "Python"),
        (Language::Rust, "Rust"),
        (Language::Js, "JavaScript"),
        (Language::Ts, "TypeScript"),
        (Language::Java, "Java"),
        (Language::Go, "Go"),
    ];

    for (lang, name) in languages {
        let separators = lang.get_separators();
        println!("\n{} has {} separators:", name, separators.len());
        println!(
            "Top separators: {:?}",
            &separators[..5.min(separators.len())]
        );
    }

    println!("\n=== Summary ===");
    println!("Code splitting helps maintain semantic boundaries in code chunks.");
    println!("Each language uses separators specific to its syntax:");
    println!("  - Python: class, def, \\tdef");
    println!("  - Rust: fn, const, let, if, while, for, loop, match");
    println!("  - JavaScript: function, const, let, var, class");
    println!("  - TypeScript: enum, interface, namespace, type, ...");
    println!("\nThis ensures that functions, classes, and control structures");
    println!("stay together when possible, improving code understanding in RAG systems.");
}
