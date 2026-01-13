//! End-to-end tests for the Superhuman Librarian
//!
//! These tests verify the librarian's core functionality without requiring
//! external services (OpenSearch, OpenAI). They test:
//! - Memory management across sessions
//! - Data structures and serialization
//!
//! Note: Tests that require OpenSearch (search, fan_out, character/theme extraction)
//! are in the unit tests within the library and require Docker infrastructure.

// `cargo verify` runs clippy with `-D warnings` for all targets, including tests.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use librarian::{
    memory::{LibrarianMemory, MemoryManager},
    RelationshipType,
};

// =============================================================================
// Memory Tests - Conversation Flow
// =============================================================================

#[test]
fn test_memory_conversation_flow() {
    let mut memory = LibrarianMemory::new("test-user");

    // Simulate a conversation about literature
    memory.add_turn(
        "What is Moby Dick about?",
        "Moby Dick is a novel by Herman Melville about Captain Ahab's obsessive quest to kill a white whale.",
        vec!["2701".to_string()],
    );
    memory.add_turn(
        "Who is the protagonist?",
        "Ishmael is the narrator and protagonist who joins the whaling ship Pequod.",
        vec!["2701".to_string()],
    );
    memory.add_turn(
        "What happened to Captain Ahab?",
        "Ahab is killed by Moby Dick in the final confrontation.",
        vec!["2701".to_string()],
    );

    assert_eq!(memory.conversation.len(), 3);
    assert!(memory.conversation[0].user.contains("Moby Dick"));
    assert!(memory.conversation[2].assistant.contains("killed"));
    assert_eq!(
        memory.conversation[0].referenced_books,
        vec!["2701".to_string()]
    );
}

#[test]
fn test_memory_conversation_limit() {
    let mut memory = LibrarianMemory::new("test-user");

    // Add more than 50 turns
    for i in 0..55 {
        memory.add_turn(format!("Question {}", i), format!("Answer {}", i), vec![]);
    }

    // Should be limited to 50
    assert_eq!(memory.conversation.len(), 50);
    // Oldest should have been removed (starts at 5)
    assert!(memory.conversation[0].user.contains("Question 5"));
}

// =============================================================================
// Memory Tests - Bookmarks
// =============================================================================

#[test]
fn test_memory_bookmark_management() {
    let mut memory = LibrarianMemory::new("test-user");

    // Add bookmarks for different books
    let bookmark1_id = memory.add_bookmark("1342", 100, Some("Great opening!".to_string()));
    let bookmark2_id = memory.add_bookmark("2701", 50, None);
    let bookmark3_id = memory.add_bookmark("1342", 500, Some("Darcy's proposal".to_string()));

    assert_eq!(memory.bookmarks.len(), 3);

    // Verify bookmark IDs are unique
    assert_ne!(bookmark1_id, bookmark2_id);
    assert_ne!(bookmark2_id, bookmark3_id);

    // Find bookmarks for a specific book
    let pride_bookmarks: Vec<_> = memory
        .bookmarks
        .iter()
        .filter(|b| b.book_id == "1342")
        .collect();
    assert_eq!(pride_bookmarks.len(), 2);
}

// =============================================================================
// Memory Tests - Notes
// =============================================================================

#[test]
fn test_memory_note_management() {
    let mut memory = LibrarianMemory::new("test-user");

    // Add notes for different books
    let note1_id = memory.add_note(
        "84",
        "The creature's eloquence is surprising and moving.",
        Some(100),
    );
    let note2_id = memory.add_note("84", "Victor's hubris is the true monster here.", Some(50));
    let _note3_id = memory.add_note("1342", "Darcy's first proposal is terrible.", None);

    assert_eq!(memory.notes.len(), 3);
    assert_ne!(note1_id, note2_id);

    // Find notes for a specific book
    let frankenstein_notes: Vec<_> = memory.notes.iter().filter(|n| n.book_id == "84").collect();
    assert_eq!(frankenstein_notes.len(), 2);
}

// =============================================================================
// Memory Tests - Reading Progress
// =============================================================================

#[test]
fn test_memory_reading_progress() {
    let mut memory = LibrarianMemory::new("test-user");

    // Start reading Moby Dick
    memory.update_reading_progress("2701", "Moby Dick", 0, Some("Chapter 1".to_string()));
    assert_eq!(memory.current_book, Some("2701".to_string()));
    assert!(memory.reading_progress.contains_key("2701"));

    let progress = memory.reading_progress.get("2701").unwrap();
    assert_eq!(progress.last_chunk_index, 0);
    assert_eq!(progress.current_chapter, Some("Chapter 1".to_string()));

    // Progress to chapter 10
    memory.update_reading_progress("2701", "Moby Dick", 250, Some("Chapter 10".to_string()));
    let progress = memory.reading_progress.get("2701").unwrap();
    assert_eq!(progress.last_chunk_index, 250);
    assert_eq!(progress.current_chapter, Some("Chapter 10".to_string()));
}

#[test]
fn test_memory_multi_book_reading() {
    let mut memory = LibrarianMemory::new("test-user");

    // Read multiple books
    memory.update_reading_progress("1342", "Pride and Prejudice", 100, None);
    memory.update_reading_progress("2701", "Moby Dick", 50, None);
    memory.update_reading_progress("84", "Frankenstein", 75, None);

    // All three should be tracked
    assert_eq!(memory.reading_progress.len(), 3);

    // Current book should be the last one updated
    assert_eq!(memory.current_book, Some("84".to_string()));

    // Each book should have its progress
    assert_eq!(
        memory
            .reading_progress
            .get("1342")
            .unwrap()
            .last_chunk_index,
        100
    );
    assert_eq!(
        memory
            .reading_progress
            .get("2701")
            .unwrap()
            .last_chunk_index,
        50
    );
    assert_eq!(
        memory.reading_progress.get("84").unwrap().last_chunk_index,
        75
    );
}

// =============================================================================
// Memory Tests - Serialization
// =============================================================================

#[test]
fn test_memory_serialization_roundtrip() {
    let mut memory = LibrarianMemory::new("serialize-test");

    // Build up state
    memory.add_turn("Q1", "A1", vec!["1342".to_string()]);
    memory.add_turn("Q2", "A2", vec![]);
    memory.add_bookmark("1342", 2000, Some("Getting good".to_string()));
    memory.update_reading_progress(
        "1342",
        "Pride and Prejudice",
        2000,
        Some("Chapter 10".to_string()),
    );
    memory.add_note("1342", "Great dialogue", Some(2000));

    // Serialize
    let json = serde_json::to_string_pretty(&memory).expect("Serialization should work");

    // Check JSON contains expected data
    assert!(json.contains("Q1"));
    assert!(json.contains("A2"));
    assert!(json.contains("Pride and Prejudice"));
    assert!(json.contains("Great dialogue"));

    // Deserialize
    let restored: LibrarianMemory =
        serde_json::from_str(&json).expect("Deserialization should work");

    // Verify state
    assert_eq!(restored.user_id, "serialize-test");
    assert_eq!(restored.conversation.len(), 2);
    assert_eq!(restored.bookmarks.len(), 1);
    assert!(restored.reading_progress.contains_key("1342"));
    assert_eq!(restored.notes.len(), 1);
}

// =============================================================================
// Telemetry Tests - LLM Call WAL Integration
// =============================================================================

#[tokio::test]
#[ignore = "requires OPENAI_API_KEY environment variable"]
async fn test_synthesis_emits_llm_telemetry_to_wal() {
    use dashflow::core::config_loader::{ChatModelConfig, SecretReference};
    use dashflow::wal::{WALEvent, WALEventType};
    use dashflow_openai::build_chat_model;
    use librarian::{AnswerSynthesizer, SearchResult};
    use std::fs;
    use std::path::Path;

    std::env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY must be set to run this ignored test");

    let wal_dir = tempfile::tempdir().expect("tempdir");
    std::env::set_var("DASHFLOW_WAL", "true");
    std::env::set_var("DASHFLOW_WAL_DIR", wal_dir.path());
    std::env::remove_var("DASHFLOW_TELEMETRY_DISABLED");

    // Use config-driven model instantiation (non-deprecated).
    let config = ChatModelConfig::OpenAI {
        model: "gpt-4o-mini".to_string(),
        api_key: SecretReference::from_env("OPENAI_API_KEY"),
        temperature: Some(0.0),
        max_tokens: Some(64),
        base_url: None,
        organization: None,
    };
    let model = build_chat_model(&config).expect("build_chat_model");

    let synthesizer = AnswerSynthesizer::new(model);
    let results = vec![SearchResult {
        content: "The narrator introduces himself as Ishmael.".to_string(),
        title: "Moby-Dick".to_string(),
        author: "Herman Melville".to_string(),
        book_id: "2701".to_string(),
        chunk_index: 0,
        score: 1.0,
    }];

    let _answer = synthesizer
        .synthesize("Who is the narrator?", &results)
        .await
        .expect("synthesize");

    fn read_wal_events_from_dir(dir: &Path) -> Vec<WALEvent> {
        let mut events = Vec::new();
        let entries = fs::read_dir(dir).expect("read_dir");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("wal") {
                continue;
            }
            let contents = fs::read_to_string(&path).expect("read wal file");
            for line in contents.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let event: WALEvent = serde_json::from_str(line).expect("parse wal event json");
                events.push(event);
            }
        }
        events
    }

    let events = read_wal_events_from_dir(wal_dir.path());
    assert!(
        events.iter().any(|e| e.event_type == WALEventType::LlmCallCompleted),
        "expected at least one llm_call_completed event in WAL; saw {:?}",
        events.iter().map(|e| e.event_type).collect::<Vec<_>>()
    );
}

// =============================================================================
// Memory Tests - Context Summary
// =============================================================================

#[test]
fn test_memory_context_summary() {
    let mut memory = LibrarianMemory::new("context-test");

    // Add conversation
    memory.add_turn(
        "What should I read?",
        "I recommend Pride and Prejudice",
        vec!["1342".to_string()],
    );

    // Set current book
    memory.update_reading_progress(
        "1342",
        "Pride and Prejudice",
        100,
        Some("Chapter 5".to_string()),
    );

    // Add bookmarks
    memory.add_bookmark("1342", 50, None);
    memory.add_bookmark("1342", 100, None);

    let summary = memory.get_context_summary();

    // Should include conversation
    assert!(summary.contains("What should I read?"));
    assert!(summary.contains("Pride and Prejudice"));

    // Should include current book
    assert!(summary.contains("Currently reading"));

    // Should include bookmark count
    assert!(summary.contains("bookmarks"));
}

// =============================================================================
// Memory Manager Tests
// =============================================================================

#[tokio::test]
async fn test_memory_manager_load_new_user() {
    let manager = MemoryManager::in_memory();

    // Load memory for new user (should create new)
    let memory = manager
        .load("new-user-123")
        .await
        .expect("Load should work");

    assert_eq!(memory.user_id, "new-user-123");
    assert!(memory.conversation.is_empty());
    assert!(memory.bookmarks.is_empty());
}

#[tokio::test]
async fn test_memory_manager_save_and_load() {
    let manager = MemoryManager::in_memory();

    // Create and modify memory
    let mut memory = manager
        .load("save-test-user")
        .await
        .expect("Load should work");
    memory.add_turn("Hello", "Hi there!", vec![]);
    memory.add_bookmark("1342", 100, None);

    // Save
    manager.save(&memory).await.expect("Save should work");

    // Clear cache to force reload
    manager.clear_cache().await;

    // In-memory manager doesn't persist to disk, so this is just testing cache
    // Load again (will come from cache in in_memory mode)
    let loaded = manager
        .load("save-test-user")
        .await
        .expect("Load should work");

    // In in_memory mode with cleared cache, it creates a new memory
    assert_eq!(loaded.user_id, "save-test-user");
}

#[tokio::test]
async fn test_memory_manager_multiple_users() {
    let manager = MemoryManager::in_memory();

    // User 1
    let mut memory1 = manager.load("user-1").await.expect("Load should work");
    memory1.add_turn("User 1 question", "User 1 answer", vec![]);
    manager.save(&memory1).await.expect("Save should work");

    // User 2
    let mut memory2 = manager.load("user-2").await.expect("Load should work");
    memory2.add_turn("User 2 question", "User 2 answer", vec![]);
    manager.save(&memory2).await.expect("Save should work");

    // Load both and verify they're separate
    let loaded1 = manager.load("user-1").await.expect("Load should work");
    let loaded2 = manager.load("user-2").await.expect("Load should work");

    assert_eq!(loaded1.conversation[0].user, "User 1 question");
    assert_eq!(loaded2.conversation[0].user, "User 2 question");
}

// =============================================================================
// Relationship Type Tests
// =============================================================================

#[test]
fn test_relationship_type_display() {
    assert_eq!(format!("{}", RelationshipType::Romantic), "Romantic");
    assert_eq!(format!("{}", RelationshipType::Family), "Family");
    assert_eq!(format!("{}", RelationshipType::Friend), "Friend");
    assert_eq!(format!("{}", RelationshipType::Antagonist), "Antagonist");
    assert_eq!(
        format!("{}", RelationshipType::Professional),
        "Professional"
    );
    assert_eq!(format!("{}", RelationshipType::Other), "Other");
}

#[test]
fn test_relationship_types_are_exhaustive() {
    // Ensure all relationship types are covered
    let types = vec![
        RelationshipType::Romantic,
        RelationshipType::Family,
        RelationshipType::Friend,
        RelationshipType::Antagonist,
        RelationshipType::Professional,
        RelationshipType::Other,
    ];

    // Verify Display is implemented for all
    for rel_type in types {
        let display = format!("{}", rel_type);
        assert!(!display.is_empty());
    }
}

// =============================================================================
// Comprehensive Session Test
// =============================================================================

#[test]
fn test_complete_reading_session() {
    let mut memory = LibrarianMemory::new("reader");

    // 1. User asks about a book
    memory.add_turn(
        "Tell me about Pride and Prejudice",
        "Pride and Prejudice is a romantic novel by Jane Austen...",
        vec!["1342".to_string()],
    );

    // 2. User starts reading
    memory.update_reading_progress(
        "1342",
        "Pride and Prejudice",
        0,
        Some("Chapter 1".to_string()),
    );
    memory.add_bookmark("1342", 0, Some("Starting the classic!".to_string()));

    // 3. User takes notes
    memory.add_note("1342", "The opening line is iconic", Some(0));

    // 4. User asks for character help
    memory.add_turn(
        "Who is Mr. Darcy?",
        "Mr. Darcy is one of the main characters...",
        vec!["1342".to_string()],
    );

    // 5. User continues reading
    memory.update_reading_progress(
        "1342",
        "Pride and Prejudice",
        500,
        Some("Chapter 20".to_string()),
    );
    memory.add_bookmark("1342", 500, Some("Halfway through!".to_string()));

    // Verify session state
    assert_eq!(memory.conversation.len(), 2);
    assert_eq!(
        memory
            .reading_progress
            .get("1342")
            .unwrap()
            .last_chunk_index,
        500
    );
    assert_eq!(memory.bookmarks.len(), 2);
    assert_eq!(memory.notes.len(), 1);
    assert_eq!(memory.current_book, Some("1342".to_string()));

    // Update favorites
    memory.update_favorites("Jane Austen");
    assert!(memory.favorite_authors.contains(&"Jane Austen".to_string()));
}

#[test]
fn test_search_history_recording() {
    let mut memory = LibrarianMemory::new("searcher");

    // Record some searches
    memory.record_search("pride and prejudice", None, 10);
    memory.record_search("whale", None, 5);
    memory.record_search("monster", None, 3);

    assert_eq!(memory.recent_searches.len(), 3);
    assert_eq!(memory.recent_searches[0].query, "pride and prejudice");
    assert_eq!(memory.recent_searches[0].result_count, 10);
}

#[test]
fn test_search_history_limit() {
    let mut memory = LibrarianMemory::new("searcher");

    // Add more than 100 searches
    for i in 0..105 {
        memory.record_search(format!("query {}", i), None, i);
    }

    // Should be limited to 100
    assert_eq!(memory.recent_searches.len(), 100);
    // Oldest should be removed
    assert!(memory.recent_searches[0].query.contains("query 5"));
}
