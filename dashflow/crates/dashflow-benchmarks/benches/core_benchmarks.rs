//! Performance benchmarks for core DashFlow functionality
//!
//! Run with: cargo bench -p dashflow-benchmarks
//! Run specific group: cargo bench -p dashflow-benchmarks text_splitters

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use dashflow::core::config::RunnableConfig;
use dashflow::core::messages::Message;
use dashflow::core::output_parsers::{
    BooleanOutputParser, CommaSeparatedListOutputParser, JsonOutputParser, LineListOutputParser,
    MarkdownListOutputParser, NumberedListOutputParser, OutputParser, RegexDictParser,
    StrOutputParser, XMLOutputParser, YamlOutputParser,
};
use dashflow::core::prompts::PromptTemplate;
use dashflow::core::runnable::{Runnable, RunnableLambda, RunnablePassthrough};
use dashflow::core::tools::{sync_function_tool, Tool};
use dashflow_text_splitters::{
    CharacterTextSplitter, RecursiveCharacterTextSplitter, TextSplitter,
};
use futures::StreamExt;
use std::collections::HashMap;

// ============================================================================
// Message Serialization
// ============================================================================

fn bench_message_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_serialization");

    // Simple human message
    group.bench_function("serialize_human_message_simple", |b| {
        b.iter(|| {
            let msg = Message::human("Hello, world!");
            serde_json::to_string(&msg).unwrap()
        });
    });

    group.bench_function("deserialize_human_message_simple", |b| {
        let json = r#"{"type":"human","content":"Hello, world!"}"#;
        b.iter(|| serde_json::from_str::<Message>(json).unwrap());
    });

    // AI message
    group.bench_function("serialize_ai_message", |b| {
        b.iter(|| {
            let msg = Message::ai("I'll search for that.");
            serde_json::to_string(&msg).unwrap()
        });
    });

    // Batch serialization
    group.bench_function("serialize_message_batch_10", |b| {
        b.iter(|| {
            let msgs: Vec<_> = (0..10)
                .map(|i| Message::human(format!("Message {}", i)))
                .collect();
            serde_json::to_string(&msgs).unwrap()
        });
    });

    group.finish();
}

// ============================================================================
// Config Operations
// ============================================================================

fn bench_config_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_operations");

    group.bench_function("create_config_with_tags", |b| {
        b.iter(|| {
            RunnableConfig::new()
                .with_tag("test")
                .with_tag("benchmark")
                .with_run_name("bench_run")
        });
    });

    group.bench_function("create_config_with_metadata", |b| {
        b.iter(|| {
            let mut config = RunnableConfig::new();
            config = config
                .with_metadata("key1", serde_json::json!("value1"))
                .unwrap();
            config = config.with_metadata("key2", serde_json::json!(42)).unwrap();
            config
        });
    });

    group.bench_function("clone_config", |b| {
        let config = RunnableConfig::new()
            .with_tag("test")
            .with_run_name("bench_run");
        b.iter(|| config.clone());
    });

    group.finish();
}

// Note: Runnable benchmarks (sequence, parallel, callback) removed for now
// These require more complex async setup with criterion that needs separate configuration

// ============================================================================
// Prompt Template Performance
// ============================================================================

fn bench_prompt_templates(c: &mut Criterion) {
    let mut group = c.benchmark_group("prompt_templates");

    // Simple template rendering - FString
    group.bench_function("render_simple_fstring", |b| {
        let template = PromptTemplate::from_template("Hello, {name}!").unwrap();
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "World".to_string());

        b.iter(|| template.format(&vars).unwrap());
    });

    // Complex template with multiple variables
    group.bench_function("render_complex_template", |b| {
        let template =
            PromptTemplate::from_template("User: {user}\nAge: {age}\nCity: {city}\nQuery: {query}")
                .unwrap();
        let mut vars = HashMap::new();
        vars.insert("user".to_string(), "Alice".to_string());
        vars.insert("age".to_string(), "30".to_string());
        vars.insert("city".to_string(), "NYC".to_string());
        vars.insert("query".to_string(), "What's the weather?".to_string());

        b.iter(|| template.format(&vars).unwrap());
    });

    // Template with long content
    group.bench_function("render_template_long_content", |b| {
        let long_text = "Lorem ipsum dolor sit amet. ".repeat(100);
        let template =
            PromptTemplate::from_template("Context: {context}\n\nQuestion: {question}").unwrap();
        let mut vars = HashMap::new();
        vars.insert("context".to_string(), long_text);
        vars.insert("question".to_string(), "Summarize".to_string());

        b.iter(|| template.format(&vars).unwrap());
    });

    group.finish();
}

// ============================================================================
// Message Operations
// ============================================================================

fn bench_message_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_operations");

    // Clone message
    group.bench_function("clone_human_message", |b| {
        let msg = Message::human("Hello, world!");
        b.iter(|| msg.clone());
    });

    // Create and clone AI message
    group.bench_function("clone_ai_message", |b| {
        let msg = Message::ai("Response from AI");
        b.iter(|| msg.clone());
    });

    group.finish();
}

// ============================================================================
// Runnable Operations
// ============================================================================

fn bench_runnable_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("runnable_operations");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Simple lambda runnable
    group.bench_function("lambda_runnable_simple", |b| {
        let lambda = RunnableLambda::new(|input: String| {
            Ok::<String, dashflow::core::error::Error>(input.to_uppercase())
        });

        b.to_async(&runtime).iter(|| async {
            lambda
                .invoke("hello world".to_string(), None)
                .await
                .unwrap()
        });
    });

    // Passthrough runnable
    group.bench_function("passthrough_runnable", |b| {
        let passthrough = RunnablePassthrough::<String>::new();

        b.to_async(&runtime).iter(|| async {
            passthrough
                .invoke("test input".to_string(), None)
                .await
                .unwrap()
        });
    });

    // Batch processing
    group.bench_function("runnable_batch_10", |b| {
        let lambda = RunnableLambda::new(|input: String| async move {
            Ok::<String, dashflow::core::error::Error>(input.to_uppercase())
        });

        let inputs: Vec<String> = (0..10).map(|i| format!("input {}", i)).collect();

        b.to_async(&runtime)
            .iter(|| async { lambda.batch(inputs.clone(), None).await.unwrap() });
    });

    group.finish();
}

// ============================================================================
// Tool Operations
// ============================================================================

fn bench_tool_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_operations");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Simple function tool
    let echo_tool = sync_function_tool(
        "echo",
        "Returns input unchanged",
        |input: String| -> Result<String, String> { Ok(input) },
    );

    group.bench_function("tool_call_simple", |b| {
        b.to_async(&runtime)
            .iter(|| async { echo_tool._call_str("test input".to_string()).await.unwrap() });
    });

    // Tool with processing
    let uppercase_tool = sync_function_tool(
        "uppercase",
        "Converts to uppercase",
        |input: String| -> Result<String, String> { Ok(input.to_uppercase()) },
    );

    group.bench_function("tool_call_with_processing", |b| {
        b.to_async(&runtime).iter(|| async {
            uppercase_tool
                ._call_str("hello world".to_string())
                .await
                .unwrap()
        });
    });

    // Tool metadata access
    group.bench_function("tool_schema_access", |b| {
        b.iter(|| {
            let _schema = echo_tool.args_schema();
            let _name = Tool::name(&echo_tool);
            let _desc = Tool::description(&echo_tool);
        });
    });

    group.finish();
}

// ============================================================================
// Streaming Operations
// ============================================================================

fn bench_streaming_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("streaming_operations");
    let runtime = tokio::runtime::Runtime::new().unwrap();

    // Passthrough streaming - simplest case
    group.bench_function("stream_passthrough", |b| {
        b.to_async(&runtime).iter(|| async {
            let passthrough = RunnablePassthrough::<String>::new();
            let mut stream = passthrough
                .stream("test input".to_string(), None)
                .await
                .unwrap();
            while (stream.next().await).is_some() {}
        });
    });

    // Passthrough batch streaming
    group.bench_function("stream_passthrough_batch_10", |b| {
        let inputs: Vec<String> = (0..10).map(|i| format!("input {}", i)).collect();

        b.to_async(&runtime).iter(|| async {
            let passthrough = RunnablePassthrough::<String>::new();
            for input in &inputs {
                let mut stream = passthrough.stream(input.clone(), None).await.unwrap();
                while (stream.next().await).is_some() {}
            }
        });
    });

    group.finish();
}

// ============================================================================
// Output Parser Performance
// ============================================================================

fn bench_output_parsers(c: &mut Criterion) {
    let mut group = c.benchmark_group("output_parsers");

    // StrOutputParser - simplest parser
    group.bench_function("str_parser_simple", |b| {
        let parser = StrOutputParser;
        let text = "Hello, world!";

        b.iter(|| parser.parse(text).unwrap());
    });

    group.bench_function("str_parser_long", |b| {
        let parser = StrOutputParser;
        let long_text = "Lorem ipsum dolor sit amet. ".repeat(100);

        b.iter(|| parser.parse(&long_text).unwrap());
    });

    // JsonOutputParser
    group.bench_function("json_parser_simple_object", |b| {
        let parser = JsonOutputParser::new();
        let json = r#"{"name": "Alice", "age": 30}"#;

        b.iter(|| parser.parse(json).unwrap());
    });

    group.bench_function("json_parser_complex_nested", |b| {
        let parser = JsonOutputParser::new();
        let json = r#"{
            "user": {"name": "Alice", "age": 30, "address": {"city": "NYC", "zip": "10001"}},
            "items": [
                {"id": 1, "name": "Item1", "price": 10.99},
                {"id": 2, "name": "Item2", "price": 20.99},
                {"id": 3, "name": "Item3", "price": 30.99}
            ],
            "metadata": {"created_at": "2025-10-31", "updated_at": "2025-10-31"}
        }"#;

        b.iter(|| parser.parse(json).unwrap());
    });

    // CommaSeparatedListOutputParser
    group.bench_function("comma_list_parser_small", |b| {
        let parser = CommaSeparatedListOutputParser;
        let text = "apple, banana, cherry";

        b.iter(|| parser.parse(text).unwrap());
    });

    group.bench_function("comma_list_parser_large", |b| {
        let parser = CommaSeparatedListOutputParser;
        let items: Vec<String> = (0..100).map(|i| format!("item{}", i)).collect();
        let list = items.join(", ");

        b.iter(|| parser.parse(&list).unwrap());
    });

    // NumberedListOutputParser
    group.bench_function("numbered_list_parser", |b| {
        let parser = NumberedListOutputParser::new();
        let text = "1. First item\n2. Second item\n3. Third item";

        b.iter(|| parser.parse(text).unwrap());
    });

    // MarkdownListOutputParser
    group.bench_function("markdown_list_parser", |b| {
        let parser = MarkdownListOutputParser::new();
        let text = "- First item\n- Second item\n- Third item";

        b.iter(|| parser.parse(text).unwrap());
    });

    // LineListOutputParser
    group.bench_function("line_list_parser", |b| {
        let parser = LineListOutputParser;
        let text = "First item\nSecond item\nThird item";

        b.iter(|| parser.parse(text).unwrap());
    });

    // BooleanOutputParser
    group.bench_function("boolean_parser_yes", |b| {
        let parser = BooleanOutputParser::new();
        let text = "YES";

        b.iter(|| parser.parse(text).unwrap());
    });

    group.bench_function("boolean_parser_no", |b| {
        let parser = BooleanOutputParser::new();
        let text = "NO";

        b.iter(|| parser.parse(text).unwrap());
    });

    // XMLOutputParser - test regex caching
    group.bench_function("xml_parser_simple", |b| {
        let parser = XMLOutputParser::new();
        let xml = "<person><name>Alice</name><age>30</age></person>";

        b.iter(|| parser.parse(xml).unwrap());
    });

    group.bench_function("xml_parser_markdown", |b| {
        let parser = XMLOutputParser::new();
        let xml = "```xml\n<person><name>Bob</name><age>25</age></person>\n```";

        b.iter(|| parser.parse(xml).unwrap());
    });

    group.bench_function("xml_parser_nested", |b| {
        let parser = XMLOutputParser::new();
        let xml = r#"
            <root>
                <user><name>Alice</name><email>alice@example.com</email></user>
                <user><name>Bob</name><email>bob@example.com</email></user>
                <user><name>Carol</name><email>carol@example.com</email></user>
            </root>
        "#;

        b.iter(|| parser.parse(xml).unwrap());
    });

    // YamlOutputParser - test regex caching
    group.bench_function("yaml_parser_simple", |b| {
        let parser = YamlOutputParser::new();
        let yaml = "name: Alice\nage: 30\ncity: NYC";

        b.iter(|| parser.parse(yaml).unwrap());
    });

    group.bench_function("yaml_parser_markdown", |b| {
        let parser = YamlOutputParser::new();
        let yaml = "```yaml\nname: Bob\nage: 25\ncity: SF\n```";

        b.iter(|| parser.parse(yaml).unwrap());
    });

    group.bench_function("yaml_parser_nested", |b| {
        let parser = YamlOutputParser::new();
        let yaml = r#"
users:
  - name: Alice
    age: 30
    address:
      city: NYC
      zip: 10001
  - name: Bob
    age: 25
    address:
      city: SF
      zip: 94102
"#;

        b.iter(|| parser.parse(yaml).unwrap());
    });

    // RegexDictParser - test regex caching (most critical - compiles regex per key!)
    group.bench_function("regex_dict_parser_single_key", |b| {
        let mut key_to_format = HashMap::new();
        key_to_format.insert("name".to_string(), "Name".to_string());
        let parser = RegexDictParser::new(key_to_format, None, None);
        let text = "Name: Alice";

        b.iter(|| parser.parse(text).unwrap());
    });

    group.bench_function("regex_dict_parser_multi_key", |b| {
        let mut key_to_format = HashMap::new();
        key_to_format.insert("name".to_string(), "Name".to_string());
        key_to_format.insert("age".to_string(), "Age".to_string());
        key_to_format.insert("city".to_string(), "City".to_string());
        key_to_format.insert("email".to_string(), "Email".to_string());
        let parser = RegexDictParser::new(key_to_format, None, None);
        let text = "Name: Alice\nAge: 30\nCity: NYC\nEmail: alice@example.com";

        b.iter(|| parser.parse(text).unwrap());
    });

    group.bench_function("regex_dict_parser_repeated_calls", |b| {
        let mut key_to_format = HashMap::new();
        key_to_format.insert("name".to_string(), "Name".to_string());
        key_to_format.insert("age".to_string(), "Age".to_string());
        let parser = RegexDictParser::new(key_to_format, None, None);

        // Test repeated calls to demonstrate caching benefit
        let texts = vec![
            "Name: Alice\nAge: 30",
            "Name: Bob\nAge: 25",
            "Name: Carol\nAge: 35",
        ];

        b.iter(|| {
            for text in &texts {
                parser.parse(text).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Text Splitter Performance
// ============================================================================

fn bench_text_splitters(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_splitters");

    // Generate test texts of various sizes
    let small_text = "This is a short paragraph. ".repeat(10); // ~260 chars
    let medium_text = "This is a medium paragraph with more content. ".repeat(100); // ~4700 chars
    let large_text = "This is a large document with lots of text. ".repeat(1000); // ~47000 chars

    // CharacterTextSplitter - small text
    group.bench_function("character_splitter_small", |b| {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(100)
            .with_chunk_overlap(20);

        b.iter(|| splitter.split_text(&small_text));
    });

    // CharacterTextSplitter - medium text
    group.bench_function("character_splitter_medium", |b| {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(500)
            .with_chunk_overlap(50);

        b.iter(|| splitter.split_text(&medium_text));
    });

    // CharacterTextSplitter - large text
    group.bench_function("character_splitter_large", |b| {
        let splitter = CharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(100);

        b.iter(|| splitter.split_text(&large_text));
    });

    // RecursiveCharacterTextSplitter - medium text
    group.bench_function("recursive_splitter_medium", |b| {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(500)
            .with_chunk_overlap(50);

        b.iter(|| splitter.split_text(&medium_text));
    });

    // RecursiveCharacterTextSplitter - large text
    group.bench_function("recursive_splitter_large", |b| {
        let splitter = RecursiveCharacterTextSplitter::new()
            .with_chunk_size(1000)
            .with_chunk_overlap(100);

        b.iter(|| splitter.split_text(&large_text));
    });

    // Compare different chunk sizes
    for &chunk_size in &[100, 500, 1000, 2000] {
        group.bench_with_input(
            BenchmarkId::new("character_splitter_varying_size", chunk_size),
            &chunk_size,
            |b, &size| {
                let splitter = CharacterTextSplitter::new()
                    .with_chunk_size(size)
                    .with_chunk_overlap(size / 10);

                b.iter(|| splitter.split_text(&medium_text));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_message_serialization,
    bench_config_operations,
    bench_prompt_templates,
    bench_message_operations,
    bench_runnable_operations,
    bench_tool_operations,
    bench_streaming_operations,
    bench_output_parsers,
    bench_text_splitters,
);
criterion_main!(benches);
