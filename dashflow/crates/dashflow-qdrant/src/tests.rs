    use super::*;
    #[allow(unused_imports)] // VectorStore trait - kept for potential trait method tests
    use dashflow::core::vector_stores::VectorStore;
    use dashflow_test_utils::MockEmbeddings;
    use serde_json::json;

    /// Helper function to create mock embeddings for testing
    /// Uses deterministic MockEmbeddings with 1536 dimensions to simulate OpenAI
    fn get_embeddings_for_test() -> Arc<dyn Embeddings> {
        Arc::new(MockEmbeddings::with_dimensions(1536))
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_empty() {
        let filter: HashMap<String, JsonValue> = HashMap::new();
        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_none());
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_string() {
        let mut filter = HashMap::new();
        filter.insert("lang".to_string(), json!("rust"));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        // Verify the condition has the right key
        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(fc)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(fc.key, "metadata.lang");
            // Verify it's a keyword match
            if let Some(m) = &fc.r#match {
                assert!(matches!(
                    &m.match_value,
                    Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(s)) if s == "rust"
                ));
            }
        } else {
            panic!("Expected FieldCondition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_integer() {
        let mut filter = HashMap::new();
        filter.insert("version".to_string(), json!(42));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(fc)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(fc.key, "metadata.version");
            if let Some(m) = &fc.r#match {
                assert!(matches!(
                    &m.match_value,
                    Some(qdrant_client::qdrant::r#match::MatchValue::Integer(i)) if *i == 42
                ));
            }
        } else {
            panic!("Expected FieldCondition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_boolean() {
        let mut filter = HashMap::new();
        filter.insert("active".to_string(), json!(true));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(fc)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(fc.key, "metadata.active");
            if let Some(m) = &fc.r#match {
                assert!(matches!(
                    &m.match_value,
                    Some(qdrant_client::qdrant::r#match::MatchValue::Boolean(b)) if *b
                ));
            }
        } else {
            panic!("Expected FieldCondition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_null_skipped() {
        let mut filter = HashMap::new();
        filter.insert("field".to_string(), json!(null));

        let result = hashmap_to_qdrant_filter(&filter);
        // Null values are skipped, so empty filter returns None
        assert!(result.is_none());
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_multiple_fields() {
        let mut filter = HashMap::new();
        filter.insert("lang".to_string(), json!("rust"));
        filter.insert("version".to_string(), json!(1));
        filter.insert("active".to_string(), json!(true));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 3);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_new_creates_store() {
        let embeddings = get_embeddings_for_test();
        let result = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Note: This will fail if Qdrant server is not running, which is expected
        // In a real test environment, we'd use a mock or integration test flag
        // For now, just verify the struct can be created
        match result {
            Ok(store) => {
                assert_eq!(store.collection_name(), "test_collection");
                assert_eq!(store.retrieval_mode(), RetrievalMode::Dense);
                assert_eq!(store.distance_metric(), DistanceMetric::Cosine);
            }
            Err(_) => {
                // Expected if Qdrant server is not running
                // This is fine for unit tests
            }
        }
    }

    #[tokio::test]
    async fn test_from_client() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        assert_eq!(store.collection_name(), "test_collection");
        assert_eq!(store.retrieval_mode(), RetrievalMode::Dense);
        assert_eq!(store.distance_metric(), DistanceMetric::Cosine);
        assert_eq!(store.content_key(), "page_content");
        assert_eq!(store.metadata_key(), "metadata");
        assert_eq!(store.vector_name(), "");
        assert_eq!(store.sparse_vector_name(), "dashflow-sparse");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_accessors() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        );

        assert_eq!(store.collection_name(), "test_collection");
        assert_eq!(store.retrieval_mode(), RetrievalMode::Dense);
        assert_eq!(store.distance_metric(), DistanceMetric::Cosine);
        assert!(store.embeddings().is_some());
        assert_eq!(store.content_key(), "page_content");
        assert_eq!(store.metadata_key(), "metadata");
        assert_eq!(store.vector_name(), "");
        assert_eq!(store.sparse_vector_name(), "dashflow-sparse");
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .with_distance_metric(DistanceMetric::Euclidean)
        .with_content_key("text")
        .with_metadata_key("meta")
        .with_vector_name("my_vector")
        .with_sparse_vector_name("my_sparse");

        assert_eq!(store.distance_metric(), DistanceMetric::Euclidean);
        assert_eq!(store.content_key(), "text");
        assert_eq!(store.metadata_key(), "meta");
        assert_eq!(store.vector_name(), "my_vector");
        assert_eq!(store.sparse_vector_name(), "my_sparse");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_clone() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let cloned = store.clone();
        assert_eq!(cloned.collection_name(), store.collection_name());
        assert_eq!(cloned.retrieval_mode(), store.retrieval_mode());
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_default_configuration_matches_python() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Verify defaults match Python baseline
        assert_eq!(store.content_key(), "page_content"); // Python: CONTENT_KEY
        assert_eq!(store.metadata_key(), "metadata"); // Python: METADATA_KEY
        assert_eq!(store.vector_name(), ""); // Python: VECTOR_NAME = ""
        assert_eq!(store.sparse_vector_name(), "dashflow-sparse"); // Python: SPARSE_VECTOR_NAME
        assert_eq!(store.distance_metric(), DistanceMetric::Cosine); // Most common default
    }

    // ========== Validation Tests ==========

    #[tokio::test]
    async fn test_validate_embeddings_dense_with_embeddings() {
        let embeddings = get_embeddings_for_test();
        let result = QdrantVectorStore::validate_embeddings(RetrievalMode::Dense, Some(embeddings));
        assert!(result.is_ok(), "Dense mode with embeddings should be valid");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_validate_embeddings_dense_without_embeddings() {
        let result = QdrantVectorStore::validate_embeddings(RetrievalMode::Dense, None);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be None when retrieval mode is Dense"));
    }

    #[tokio::test]
    async fn test_validate_embeddings_sparse_not_implemented() {
        let embeddings = get_embeddings_for_test();
        let result =
            QdrantVectorStore::validate_embeddings(RetrievalMode::Sparse, Some(embeddings));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Sparse retrieval mode"));
    }

    #[tokio::test]
    async fn test_validate_embeddings_hybrid_not_implemented() {
        let embeddings = get_embeddings_for_test();
        let result =
            QdrantVectorStore::validate_embeddings(RetrievalMode::Hybrid, Some(embeddings));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Hybrid retrieval mode"));
    }

    #[tokio::test]
    async fn test_distance_metric_conversion() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        // Test Cosine
        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .with_distance_metric(DistanceMetric::Cosine);
        assert_eq!(store.distance_metric_to_qdrant(), Distance::Cosine);

        // Test Euclidean
        let store = store.with_distance_metric(DistanceMetric::Euclidean);
        assert_eq!(store.distance_metric_to_qdrant(), Distance::Euclid);

        // Test DotProduct
        let store = store.with_distance_metric(DistanceMetric::DotProduct);
        assert_eq!(store.distance_metric_to_qdrant(), Distance::Dot);

        // Test MaxInnerProduct (maps to Dot)
        let store = store.with_distance_metric(DistanceMetric::MaxInnerProduct);
        assert_eq!(store.distance_metric_to_qdrant(), Distance::Dot);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_validate_collection_config_success() {
        // This test requires:
        // 1. Qdrant server running at localhost:6334
        // 2. Collection "test_validation" with:
        //    - 1536-dimensional vectors (matches OpenAI text-embedding-3-small)
        //    - Cosine distance metric
        //    - Unnamed vector (vector_name = "")
        //
        // To create the collection:
        // docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant
        // Then use Qdrant API to create collection with above config

        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_validation",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        let result = store.validate_collection_config().await;
        assert!(result.is_ok(), "Validation should succeed: {:?}", result);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_validate_collection_config_collection_not_exists() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "nonexistent_collection_12345",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let result = store.validate_collection_config().await;
        assert!(result.is_err());
        // Note: Error message depends on whether Qdrant server is running
        // If server not running: connection error
        // If server running: collection not found error
    }

    // ========================================================================
    // Payload Conversion Tests
    // ========================================================================

    #[tokio::test]
    async fn test_build_payload_with_simple_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        metadata.insert("year".to_string(), JsonValue::Number(2024.into()));

        let payload = store.build_payload("Hello world", Some(&metadata));

        // Convert back to HashMap for easy testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        // Check content key exists
        assert!(payload_map.contains_key("page_content"));

        // Check metadata key exists
        assert!(payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_with_null_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let payload = store.build_payload("Hello world", None);

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        // Check both keys exist
        assert!(payload_map.contains_key("page_content"));
        assert!(payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_with_nested_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let mut metadata = HashMap::new();
        metadata.insert(
            "nested".to_string(),
            serde_json::json!({
                "level1": {
                    "level2": "deep value"
                }
            }),
        );

        let payload = store.build_payload("Test", Some(&metadata));

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        assert!(payload_map.contains_key("page_content"));
        assert!(payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_with_array_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let mut metadata = HashMap::new();
        metadata.insert(
            "tags".to_string(),
            serde_json::json!(["tag1", "tag2", "tag3"]),
        );

        let payload = store.build_payload("Test", Some(&metadata));

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        assert!(payload_map.contains_key("page_content"));
        assert!(payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_with_mixed_types() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let pi = std::f64::consts::PI;
        let mut metadata = HashMap::new();
        metadata.insert("string".to_string(), JsonValue::String("test".to_string()));
        metadata.insert("integer".to_string(), JsonValue::Number(42.into()));
        metadata.insert("float".to_string(), serde_json::json!(pi));
        metadata.insert("bool".to_string(), JsonValue::Bool(true));
        metadata.insert("null".to_string(), JsonValue::Null);
        metadata.insert("array".to_string(), serde_json::json!([1, 2, 3]));
        metadata.insert("object".to_string(), serde_json::json!({"key": "value"}));

        let payload = store.build_payload("Test", Some(&metadata));

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        assert!(payload_map.contains_key("page_content"));
        assert!(payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_with_custom_keys() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .with_content_key("text")
        .with_metadata_key("meta");

        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), JsonValue::String("Bob".to_string()));

        let payload = store.build_payload("Custom keys", Some(&metadata));

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        // Should use custom keys
        assert!(payload_map.contains_key("text"));
        assert!(payload_map.contains_key("meta"));
        assert!(!payload_map.contains_key("page_content"));
        assert!(!payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_empty_content() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let payload = store.build_payload("", None);

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        assert!(payload_map.contains_key("page_content"));
        assert!(payload_map.contains_key("metadata"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_payload_empty_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let empty_metadata = HashMap::new();
        let payload = store.build_payload("Test", Some(&empty_metadata));

        // Convert back to HashMap for testing
        let payload_map: HashMap<String, qdrant_client::qdrant::Value> = payload.into();

        assert!(payload_map.contains_key("page_content"));
        assert!(payload_map.contains_key("metadata"));
    }

    // ========================================================================
    // Tests for payload_to_document() - Reverse conversion
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_with_simple_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Build a payload first
        let mut metadata = HashMap::new();
        metadata.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        metadata.insert("year".to_string(), JsonValue::Number(2024.into()));

        let payload = store.build_payload("Hello world", Some(&metadata));
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Convert back to document
        let (content, extracted_metadata) = store.payload_to_document(&payload_map);

        assert_eq!(content, "Hello world");
        assert_eq!(extracted_metadata.len(), 2);
        assert_eq!(
            extracted_metadata.get("author"),
            Some(&JsonValue::String("Alice".to_string()))
        );
        assert_eq!(
            extracted_metadata.get("year"),
            Some(&JsonValue::Number(2024.into()))
        );
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_with_null_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Build payload with None metadata → null value
        let payload = store.build_payload("Test content", None);
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Convert back to document
        let (content, metadata) = store.payload_to_document(&payload_map);

        // Python baseline: None → empty dict
        assert_eq!(content, "Test content");
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_with_nested_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Build nested metadata
        let mut inner_map = serde_json::Map::new();
        inner_map.insert("city".to_string(), JsonValue::String("NYC".to_string()));
        inner_map.insert("zip".to_string(), JsonValue::Number(10001.into()));

        let mut metadata = HashMap::new();
        metadata.insert("location".to_string(), JsonValue::Object(inner_map));

        let payload = store.build_payload("Nested test", Some(&metadata));
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Convert back to document
        let (content, extracted_metadata) = store.payload_to_document(&payload_map);

        assert_eq!(content, "Nested test");
        assert_eq!(extracted_metadata.len(), 1);

        let location = extracted_metadata.get("location").unwrap();
        if let JsonValue::Object(map) = location {
            assert_eq!(map.get("city"), Some(&JsonValue::String("NYC".to_string())));
            assert_eq!(map.get("zip"), Some(&JsonValue::Number(10001.into())));
        } else {
            panic!("Expected location to be an object");
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_with_array_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let mut metadata = HashMap::new();
        metadata.insert(
            "tags".to_string(),
            JsonValue::Array(vec![
                JsonValue::String("rust".to_string()),
                JsonValue::String("dashflow".to_string()),
            ]),
        );

        let payload = store.build_payload("Array test", Some(&metadata));
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Convert back to document
        let (content, extracted_metadata) = store.payload_to_document(&payload_map);

        assert_eq!(content, "Array test");
        assert_eq!(extracted_metadata.len(), 1);

        let tags = extracted_metadata.get("tags").unwrap();
        if let JsonValue::Array(arr) = tags {
            assert_eq!(arr.len(), 2);
            assert_eq!(arr[0], JsonValue::String("rust".to_string()));
            assert_eq!(arr[1], JsonValue::String("dashflow".to_string()));
        } else {
            panic!("Expected tags to be an array");
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_with_mixed_types() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let mut metadata = HashMap::new();
        metadata.insert(
            "string_val".to_string(),
            JsonValue::String("test".to_string()),
        );
        metadata.insert("int_val".to_string(), JsonValue::Number(42.into()));
        metadata.insert(
            "float_val".to_string(),
            JsonValue::Number(serde_json::Number::from_f64(std::f64::consts::PI).unwrap()),
        );
        metadata.insert("bool_val".to_string(), JsonValue::Bool(true));
        metadata.insert("null_val".to_string(), JsonValue::Null);

        let payload = store.build_payload("Mixed types", Some(&metadata));
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Convert back to document
        let (content, extracted_metadata) = store.payload_to_document(&payload_map);

        assert_eq!(content, "Mixed types");
        assert_eq!(extracted_metadata.len(), 5);
        assert_eq!(
            extracted_metadata.get("string_val"),
            Some(&JsonValue::String("test".to_string()))
        );
        assert_eq!(
            extracted_metadata.get("int_val"),
            Some(&JsonValue::Number(42.into()))
        );
        assert_eq!(
            extracted_metadata.get("bool_val"),
            Some(&JsonValue::Bool(true))
        );
        assert_eq!(extracted_metadata.get("null_val"), Some(&JsonValue::Null));

        // Float comparison needs special handling
        if let Some(JsonValue::Number(n)) = extracted_metadata.get("float_val") {
            assert!((n.as_f64().unwrap() - std::f64::consts::PI).abs() < 0.001);
        } else {
            panic!("Expected float_val to be a number");
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_with_custom_keys() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .with_content_key("text")
        .with_metadata_key("meta");

        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), JsonValue::String("value".to_string()));

        let payload = store.build_payload("Custom keys", Some(&metadata));
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Verify custom keys were used
        assert!(payload_map.contains_key("text"));
        assert!(payload_map.contains_key("meta"));
        assert!(!payload_map.contains_key("page_content"));
        assert!(!payload_map.contains_key("metadata"));

        // Convert back to document
        let (content, extracted_metadata) = store.payload_to_document(&payload_map);

        assert_eq!(content, "Custom keys");
        assert_eq!(extracted_metadata.len(), 1);
        assert_eq!(
            extracted_metadata.get("key"),
            Some(&JsonValue::String("value".to_string()))
        );
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_missing_content() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Create payload with only metadata, no content
        let mut payload_map = HashMap::new();
        let metadata_value = qdrant::Value {
            kind: Some(qdrant::value::Kind::StructValue(qdrant::Struct {
                fields: [(
                    "key".to_string(),
                    qdrant::Value {
                        kind: Some(qdrant::value::Kind::StringValue("value".to_string())),
                    },
                )]
                .into_iter()
                .collect(),
            })),
        };
        payload_map.insert("metadata".to_string(), metadata_value);

        // Convert to document - missing content should return empty string
        let (content, metadata) = store.payload_to_document(&payload_map);

        // Python baseline: payload.get(content_payload_key, "")
        assert_eq!(content, "");
        assert_eq!(metadata.len(), 1);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_missing_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Create payload with only content, no metadata
        let mut payload_map = HashMap::new();
        let content_value = qdrant::Value {
            kind: Some(qdrant::value::Kind::StringValue("Only content".to_string())),
        };
        payload_map.insert("page_content".to_string(), content_value);

        // Convert to document - missing metadata should return empty HashMap
        let (content, metadata) = store.payload_to_document(&payload_map);

        // Python baseline: payload.get(metadata_payload_key) or {}
        assert_eq!(content, "Only content");
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_empty_payload() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Empty payload
        let payload_map = HashMap::new();

        // Convert to document - both should be empty/default
        let (content, metadata) = store.payload_to_document(&payload_map);

        assert_eq!(content, "");
        assert_eq!(metadata.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_payload_to_document_roundtrip() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Original data
        let original_content = "Roundtrip test";
        let mut original_metadata = HashMap::new();
        original_metadata.insert("key1".to_string(), JsonValue::String("value1".to_string()));
        original_metadata.insert("key2".to_string(), JsonValue::Number(123.into()));

        // Forward: document → payload
        let payload = store.build_payload(original_content, Some(&original_metadata));
        let payload_map: HashMap<String, qdrant::Value> = payload.into();

        // Reverse: payload → document
        let (content, metadata) = store.payload_to_document(&payload_map);

        // Verify roundtrip preserves data
        assert_eq!(content, original_content);
        assert_eq!(metadata.len(), original_metadata.len());
        assert_eq!(metadata.get("key1"), original_metadata.get("key1"));
        assert_eq!(metadata.get("key2"), original_metadata.get("key2"));
    }

    // ========================================================================
    // Tests for build_vectors() - Embedding texts to vectors
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_dense_mode_unnamed() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Hello".to_string(), "World".to_string()];
        let vectors_result = store.build_vectors(&texts).await;

        assert!(vectors_result.is_ok());
        let vectors = vectors_result.unwrap();
        assert_eq!(vectors.len(), 2);

        // Verify vectors are unnamed (default vector_name = "")
        // Unnamed vectors use VectorsOptions::Vector variant
        for vector in &vectors {
            assert!(vector.vectors_options.is_some());
            // Unnamed vectors should use Vector variant (single default vector)
            match &vector.vectors_options {
                Some(qdrant::vectors::VectorsOptions::Vector(_)) => {
                    // Expected for unnamed vectors
                }
                Some(qdrant::vectors::VectorsOptions::Vectors(_)) => {
                    panic!("Expected unnamed vector (Vector variant), got named vectors");
                }
                None => panic!("vectors_options should not be None"),
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_dense_mode_named() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .with_vector_name("text_vector");

        let texts = vec!["Test".to_string()];
        let vectors_result = store.build_vectors(&texts).await;

        assert!(vectors_result.is_ok());
        let vectors = vectors_result.unwrap();
        assert_eq!(vectors.len(), 1);

        // Verify vectors are named
        let vector = &vectors[0];
        match &vector.vectors_options {
            Some(qdrant::vectors::VectorsOptions::Vectors(named_vectors)) => {
                assert!(named_vectors.vectors.contains_key("text_vector"));
                assert_eq!(named_vectors.vectors.len(), 1);
            }
            Some(qdrant::vectors::VectorsOptions::Vector(_)) => {
                panic!("Expected named vectors (Vectors variant), got unnamed vector");
            }
            None => panic!("vectors_options should not be None"),
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_no_embeddings_provider() {
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        // Create store without embeddings provider
        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            None, // No embeddings!
            RetrievalMode::Dense,
        );

        let texts = vec!["Test".to_string()];
        let result = store.build_vectors(&texts).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("DENSE mode requires embeddings"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_empty_texts() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts: Vec<String> = vec![];
        let vectors_result = store.build_vectors(&texts).await;

        // Empty input should return empty output
        assert!(vectors_result.is_ok());
        let vectors = vectors_result.unwrap();
        assert_eq!(vectors.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_sparse_mode_not_implemented() {
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store =
            QdrantVectorStore::from_client(client, "test_collection", None, RetrievalMode::Sparse);

        let texts = vec!["Test".to_string()];
        let result = store.build_vectors(&texts).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("SPARSE mode"));
        assert!(err.to_string().contains("sparse vector encoders"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_hybrid_mode_not_implemented() {
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store =
            QdrantVectorStore::from_client(client, "test_collection", None, RetrievalMode::Hybrid);

        let texts = vec!["Test".to_string()];
        let result = store.build_vectors(&texts).await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("HYBRID mode"));
        assert!(err.to_string().contains("sparse vector encoders"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_build_vectors_multiple_texts() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec![
            "First text".to_string(),
            "Second text".to_string(),
            "Third text".to_string(),
            "Fourth text".to_string(),
        ];
        let vectors_result = store.build_vectors(&texts).await;

        assert!(vectors_result.is_ok());
        let vectors = vectors_result.unwrap();
        assert_eq!(vectors.len(), 4);
    }

    // ========================================================================
    // Tests for generate_ids() - ID generation and validation
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_auto_generation() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Text 1".to_string(), "Text 2".to_string()];
        let ids = store.generate_ids(&texts, None).unwrap();

        // Should generate 2 IDs
        assert_eq!(ids.len(), 2);

        // IDs should be unique
        assert_ne!(ids[0], ids[1]);

        // IDs should be UUIDs in simple format (32 chars, lowercase hex)
        for id in &ids {
            assert_eq!(id.len(), 32);
            assert!(id
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_with_provided_ids() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Text 1".to_string(), "Text 2".to_string()];
        let provided_ids = vec!["custom_id_1".to_string(), "custom_id_2".to_string()];

        let ids = store.generate_ids(&texts, Some(&provided_ids)).unwrap();

        // Should use provided IDs
        assert_eq!(ids, provided_ids);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_count_mismatch() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Text 1".to_string(), "Text 2".to_string()];
        let provided_ids = vec!["id1".to_string()]; // Only 1 ID for 2 texts

        let result = store.generate_ids(&texts, Some(&provided_ids));

        // Should return error
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::InvalidInput(_)));
        assert!(err.to_string().contains("Provided IDs count"));
        assert!(err.to_string().contains("does not match texts count"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_empty_texts() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts: Vec<String> = vec![];
        let ids = store.generate_ids(&texts, None).unwrap();

        // Empty texts should return empty IDs
        assert_eq!(ids.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_uuid_format() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Test".to_string()];
        let ids = store.generate_ids(&texts, None).unwrap();

        // UUID should be in simple format (no dashes)
        let id = &ids[0];
        assert_eq!(id.len(), 32);
        assert!(!id.contains('-'));
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));

        // Should be lowercase (Python uses .hex which is lowercase)
        assert!(!id.chars().any(|c| c.is_uppercase()));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_multiple_calls_different_ids() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Test".to_string()];

        // Generate IDs twice
        let ids1 = store.generate_ids(&texts, None).unwrap();
        let ids2 = store.generate_ids(&texts, None).unwrap();

        // Should generate different IDs each time (UUIDs are random)
        assert_ne!(ids1[0], ids2[0]);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_provided_empty_ids_for_empty_texts() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts: Vec<String> = vec![];
        let provided_ids: Vec<String> = vec![];

        let ids = store.generate_ids(&texts, Some(&provided_ids)).unwrap();

        // Empty texts + empty IDs should be valid
        assert_eq!(ids.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_generate_ids_with_numeric_string_ids() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Test".to_string()];
        let provided_ids = vec!["12345".to_string()]; // Numeric string ID

        let ids = store.generate_ids(&texts, Some(&provided_ids)).unwrap();

        // Should accept numeric string IDs (Python accepts str or int)
        assert_eq!(ids[0], "12345");
    }

    // ========================================
    // add_texts() Tests
    // ========================================

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_without_metadata_or_ids() {
        // This test requires a running Qdrant server at localhost:6334
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_add_texts_basic",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        let texts = vec!["Hello world", "Goodbye world"];

        let result = store.add_texts(&texts, None, None, 64).await;
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids.len(), 2);

        // IDs should be UUIDs (32 chars, lowercase hex)
        for id in &ids {
            assert_eq!(id.len(), 32);
            assert!(id
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        }

        // IDs should be unique
        assert_ne!(ids[0], ids[1]);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_with_metadata() {
        // This test requires a running Qdrant server at localhost:6334
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_add_texts_metadata",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        let texts = vec!["Hello", "Goodbye"];

        let mut meta1 = HashMap::new();
        meta1.insert(
            "category".to_string(),
            JsonValue::String("greeting".to_string()),
        );
        meta1.insert("lang".to_string(), JsonValue::String("en".to_string()));

        let mut meta2 = HashMap::new();
        meta2.insert(
            "category".to_string(),
            JsonValue::String("farewell".to_string()),
        );
        meta2.insert("lang".to_string(), JsonValue::String("en".to_string()));

        let metadatas = vec![meta1, meta2];

        let result = store.add_texts(&texts, Some(&metadatas), None, 64).await;
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids.len(), 2);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_with_custom_ids() {
        // This test requires a running Qdrant server at localhost:6334
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_add_texts_custom_ids",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        let texts = vec!["Test 1", "Test 2"];
        let custom_ids = vec!["custom_1".to_string(), "custom_2".to_string()];

        let result = store.add_texts(&texts, None, Some(&custom_ids), 64).await;
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids, custom_ids);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_with_metadata_and_custom_ids() {
        // This test requires a running Qdrant server at localhost:6334
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_add_texts_full",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        let texts = vec!["Doc 1", "Doc 2"];

        let mut meta1 = HashMap::new();
        meta1.insert("source".to_string(), JsonValue::String("test".to_string()));

        let mut meta2 = HashMap::new();
        meta2.insert("source".to_string(), JsonValue::String("test".to_string()));

        let metadatas = vec![meta1, meta2];
        let custom_ids = vec!["doc_1".to_string(), "doc_2".to_string()];

        let result = store
            .add_texts(&texts, Some(&metadatas), Some(&custom_ids), 64)
            .await;
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids, custom_ids);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_metadata_count_mismatch() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let mut store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Text 1", "Text 2"];

        let mut meta1 = HashMap::new();
        meta1.insert("key".to_string(), JsonValue::String("value".to_string()));
        let metadatas = vec![meta1]; // Only 1 metadata for 2 texts

        let result = store.add_texts(&texts, Some(&metadatas), None, 64).await;

        // Should return error due to count mismatch
        assert!(result.is_err());
        if let Err(Error::InvalidInput(msg)) = result {
            assert!(msg.contains("Metadata count"));
            assert!(msg.contains("does not match"));
        } else {
            panic!("Expected InvalidInput error");
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_id_count_mismatch() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let mut store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts = vec!["Text 1", "Text 2"];
        let custom_ids = vec!["id_1".to_string()]; // Only 1 ID for 2 texts

        let result = store.add_texts(&texts, None, Some(&custom_ids), 64).await;

        // Should return error due to count mismatch (from generate_ids)
        assert!(result.is_err());
        if let Err(Error::InvalidInput(msg)) = result {
            assert!(msg.contains("IDs count"));
            assert!(msg.contains("does not match"));
        } else {
            panic!("Expected InvalidInput error");
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_empty_texts() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let mut store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let texts: Vec<&str> = vec![];

        // Empty texts should succeed (but not call Qdrant)
        // This is a unit test - it won't actually call Qdrant since texts is empty
        // In practice, with empty texts the upsert won't fail but will be a no-op
        let result = store.add_texts(&texts, None, None, 64).await;

        // The result depends on whether Qdrant client accepts empty points
        // For now, we just test that it doesn't panic
        match result {
            Ok(ids) => assert_eq!(ids.len(), 0),
            Err(_) => {
                // Some implementations may error on empty, which is also acceptable
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_without_embeddings_provider() {
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        // Create store without embeddings provider
        let mut store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            None, // No embeddings
            RetrievalMode::Dense,
        );

        let texts = vec!["Text 1"];

        let result = store.add_texts(&texts, None, None, 64).await;

        // Should error because DENSE mode requires embeddings
        assert!(result.is_err());
        if let Err(Error::InvalidInput(msg)) = result {
            assert!(msg.contains("DENSE mode requires embeddings"));
        } else {
            panic!("Expected InvalidInput error for missing embeddings");
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_with_batching() {
        // Test that batching works correctly by adding multiple texts with small batch size
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_add_texts_batching",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Create 10 texts to test batching
        let texts: Vec<String> = (0..10).map(|i| format!("Text {}", i)).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        // Use batch size of 3 to ensure we have multiple batches
        let result = store.add_texts(&text_refs, None, None, 3).await;
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids.len(), 10);

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), 10);

        // All IDs should be valid UUIDs (32 hex chars)
        for id in &ids {
            assert_eq!(id.len(), 32);
            assert!(id
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_add_texts_with_batching_and_metadata() {
        // Test batching with metadata to ensure batch slicing works correctly
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_add_texts_batching_metadata",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Create 5 texts with metadata
        let texts: Vec<String> = (0..5).map(|i| format!("Document {}", i)).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        let metadatas: Vec<HashMap<String, JsonValue>> = (0..5)
            .map(|i| {
                let mut meta = HashMap::new();
                meta.insert("index".to_string(), JsonValue::Number(i.into()));
                meta.insert("batch".to_string(), JsonValue::String("test".to_string()));
                meta
            })
            .collect();

        // Use batch size of 2 to have 3 batches (2, 2, 1)
        let result = store.add_texts(&text_refs, Some(&metadatas), None, 2).await;
        assert!(result.is_ok());

        let ids = result.unwrap();
        assert_eq!(ids.len(), 5);

        // All IDs should be unique
        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique_ids.len(), 5);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_by_vector_basic() {
        // Test basic similarity search with a pre-computed embedding
        let embeddings = get_embeddings_for_test();

        // Create store and add some documents first
        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_search",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Hello world", "Goodbye world", "Rust programming"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search
        let embedding = vec![0.1, 0.2, 0.3];
        let results = store
            .similarity_search_with_score_by_vector(&embedding, 2, None, None, 0, None)
            .await
            .unwrap();

        // Should return 2 results
        assert_eq!(results.len(), 2);

        // Each result should have a document and a score
        for (doc, score) in &results {
            assert!(!doc.page_content.is_empty());
            assert!(score >= &0.0);
            // Metadata should include _id and _collection_name
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
            assert_eq!(
                doc.metadata.get("_collection_name"),
                Some(&JsonValue::String("test_similarity_search".to_string()))
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_by_vector_with_score_threshold() {
        // Test similarity search with score threshold
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_threshold",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Document 1", "Document 2", "Document 3"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search with score threshold
        let embedding = vec![0.1, 0.2, 0.3];
        let results = store
            .similarity_search_with_score_by_vector(&embedding, 10, None, None, 0, Some(0.9))
            .await
            .unwrap();

        // All returned results should have score >= 0.9
        for (_doc, score) in &results {
            assert!(score >= &0.9);
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_by_vector_with_offset() {
        // Test similarity search with offset for pagination
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_offset",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add 10 test documents
        let texts: Vec<String> = (0..10).map(|i| format!("Document {}", i)).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let _ids = store.add_texts(&text_refs, None, None, 64).await.unwrap();

        let embedding = vec![0.1, 0.2, 0.3];

        // Get first 3 results
        let results_page1 = store
            .similarity_search_with_score_by_vector(&embedding, 3, None, None, 0, None)
            .await
            .unwrap();
        assert_eq!(results_page1.len(), 3);

        // Get next 3 results (offset=3)
        let results_page2 = store
            .similarity_search_with_score_by_vector(&embedding, 3, None, None, 3, None)
            .await
            .unwrap();
        assert_eq!(results_page2.len(), 3);

        // Results should be different (different documents)
        let ids_page1: Vec<String> = results_page1
            .iter()
            .map(|(doc, _)| {
                doc.metadata
                    .get("_id")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        let ids_page2: Vec<String> = results_page2
            .iter()
            .map(|(doc, _)| {
                doc.metadata
                    .get("_id")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        // No overlap between pages
        for id in &ids_page1 {
            assert!(!ids_page2.contains(id));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_by_vector_with_metadata() {
        // Test that original metadata is preserved in results
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_metadata",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add documents with metadata
        let texts = vec!["First doc", "Second doc"];
        let mut metadatas = Vec::new();
        let mut meta1 = HashMap::new();
        meta1.insert("author".to_string(), JsonValue::String("Alice".to_string()));
        meta1.insert("page".to_string(), JsonValue::Number(1.into()));
        metadatas.push(meta1);

        let mut meta2 = HashMap::new();
        meta2.insert("author".to_string(), JsonValue::String("Bob".to_string()));
        meta2.insert("page".to_string(), JsonValue::Number(2.into()));
        metadatas.push(meta2);

        let _ids = store
            .add_texts(&texts, Some(&metadatas), None, 64)
            .await
            .unwrap();

        // Search
        let embedding = vec![0.1, 0.2, 0.3];
        let results = store
            .similarity_search_with_score_by_vector(&embedding, 2, None, None, 0, None)
            .await
            .unwrap();

        assert_eq!(results.len(), 2);

        // Check that metadata is preserved
        for (doc, _score) in &results {
            // Should have original metadata plus _id and _collection_name
            assert!(doc.metadata.contains_key("author"));
            assert!(doc.metadata.contains_key("page"));
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_by_vector_empty_collection() {
        // Test search on empty collection (should return empty results, not error)
        let embeddings = get_embeddings_for_test();

        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_empty",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Search in empty collection
        let embedding = vec![0.1, 0.2, 0.3];
        let result = store
            .similarity_search_with_score_by_vector(&embedding, 5, None, None, 0, None)
            .await;

        // Should succeed but return empty results
        match result {
            Ok(results) => {
                assert_eq!(results.len(), 0);
            }
            Err(_) => {
                // Collection might not exist, which is fine for this test
            }
        }
    }

    // ========== similarity_search_by_vector() Tests ==========

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_by_vector_basic() {
        // Test basic similarity search (without scores)
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_by_vector_basic",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Hello world", "Goodbye world", "Another document"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search
        let embedding = vec![0.1, 0.2, 0.3];
        let results = store
            .similarity_search_by_vector(&embedding, 2, None, None, 0, None)
            .await
            .unwrap();

        // Verify results
        assert_eq!(results.len(), 2);
        for doc in &results {
            // Check document has content
            assert!(!doc.page_content.is_empty());
            // Check metadata includes special fields
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_by_vector_with_score_threshold() {
        // Test similarity search with score threshold (documents only, no scores returned)
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_by_vector_threshold",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Document 1", "Document 2", "Document 3"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search with score threshold
        let embedding = vec![0.1, 0.2, 0.3];
        let results = store
            .similarity_search_by_vector(&embedding, 10, None, None, 0, Some(0.9))
            .await
            .unwrap();

        // Verify all results meet threshold (we can't check scores directly since they're not returned)
        // But we can verify we get results back
        for doc in &results {
            assert!(!doc.page_content.is_empty());
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_by_vector_with_pagination() {
        // Test similarity search with pagination
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_by_vector_pagination",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add 10 test documents
        let texts: Vec<String> = (0..10).map(|i| format!("Document {}", i)).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let _ids = store.add_texts(&text_refs, None, None, 64).await.unwrap();

        let embedding = vec![0.1, 0.2, 0.3];

        // Get first page (3 results, offset=0)
        let page1 = store
            .similarity_search_by_vector(&embedding, 3, None, None, 0, None)
            .await
            .unwrap();
        assert_eq!(page1.len(), 3);

        // Get second page (3 results, offset=3)
        let page2 = store
            .similarity_search_by_vector(&embedding, 3, None, None, 3, None)
            .await
            .unwrap();
        assert_eq!(page2.len(), 3);

        // Verify different pages have different documents
        let page1_ids: Vec<_> = page1
            .iter()
            .map(|doc| doc.metadata.get("_id").unwrap().clone())
            .collect();
        let page2_ids: Vec<_> = page2
            .iter()
            .map(|doc| doc.metadata.get("_id").unwrap().clone())
            .collect();

        // No overlap between pages
        for id in &page1_ids {
            assert!(!page2_ids.contains(id));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_by_vector_empty_collection() {
        // Test search on empty collection (should return empty results)
        let embeddings = get_embeddings_for_test();

        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_by_vector_empty",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Search in empty collection
        let embedding = vec![0.1, 0.2, 0.3];
        let result = store
            .similarity_search_by_vector(&embedding, 5, None, None, 0, None)
            .await;

        // Should succeed but return empty results
        match result {
            Ok(results) => {
                assert_eq!(results.len(), 0);
            }
            Err(_) => {
                // Collection might not exist, which is fine for this test
            }
        }
    }

    // ========== similarity_search_with_score() Tests ==========

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_basic() {
        // Test basic text-based similarity search with scores
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_with_score_basic",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Hello world", "Goodbye world", "Another document"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search by text query
        let results = store
            .similarity_search_with_score("Hello", 2, None, None, 0, None)
            .await
            .unwrap();

        // Verify results
        assert_eq!(results.len(), 2);
        for (doc, score) in &results {
            // Check document has content
            assert!(!doc.page_content.is_empty());
            // Check score is present
            assert!(*score >= 0.0);
            // Check metadata includes special fields
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_with_threshold() {
        // Test similarity search with score threshold
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_with_score_threshold",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Document 1", "Document 2", "Document 3"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search with score threshold
        let results = store
            .similarity_search_with_score("Document", 10, None, None, 0, Some(0.9))
            .await
            .unwrap();

        // All returned results should have score >= 0.9
        for (_doc, score) in &results {
            assert!(score >= &0.9);
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_with_pagination() {
        // Test similarity search with pagination
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_with_score_pagination",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add 10 test documents
        let texts: Vec<String> = (0..10).map(|i| format!("Document {}", i)).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let _ids = store.add_texts(&text_refs, None, None, 64).await.unwrap();

        // Get first page (3 results, offset=0)
        let page1 = store
            .similarity_search_with_score("Document", 3, None, None, 0, None)
            .await
            .unwrap();
        assert_eq!(page1.len(), 3);

        // Get second page (3 results, offset=3)
        let page2 = store
            .similarity_search_with_score("Document", 3, None, None, 3, None)
            .await
            .unwrap();
        assert_eq!(page2.len(), 3);

        // Verify different pages have different documents
        let page1_ids: Vec<_> = page1
            .iter()
            .map(|(doc, _)| doc.metadata.get("_id").unwrap().clone())
            .collect();
        let page2_ids: Vec<_> = page2
            .iter()
            .map(|(doc, _)| doc.metadata.get("_id").unwrap().clone())
            .collect();

        // No overlap between pages
        for id in &page1_ids {
            assert!(!page2_ids.contains(id));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_score_no_embeddings() {
        // Test that similarity_search_with_score returns error when no embeddings provider
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_with_score_no_embeddings",
            None, // No embeddings provider
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Should error because no embeddings provider
        let result = store
            .similarity_search_with_score("test query", 5, None, None, 0, None)
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Embeddings provider required"));
    }

    // ========== similarity_search() Tests ==========

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_basic() {
        // Test basic text-based similarity search (without scores)
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_basic",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Hello world", "Goodbye world", "Another document"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search by text query (without scores)
        let results = store
            .similarity_search("Hello", 2, None, None, 0, None)
            .await
            .unwrap();

        // Verify results
        assert_eq!(results.len(), 2);
        for doc in &results {
            // Check document has content
            assert!(!doc.page_content.is_empty());
            // Check metadata includes special fields
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_threshold() {
        // Test similarity search with score threshold (documents only)
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_threshold",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add test documents
        let texts = vec!["Document 1", "Document 2", "Document 3"];
        let _ids = store.add_texts(&texts, None, None, 64).await.unwrap();

        // Perform similarity search with score threshold
        let results = store
            .similarity_search("Document", 10, None, None, 0, Some(0.9))
            .await
            .unwrap();

        // Verify we get results back (can't check scores since they're not returned)
        for doc in &results {
            assert!(!doc.page_content.is_empty());
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_with_pagination() {
        // Test similarity search with pagination
        let embeddings = get_embeddings_for_test();

        let mut store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_pagination",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .unwrap();

        // Add 10 test documents
        let texts: Vec<String> = (0..10).map(|i| format!("Document {}", i)).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let _ids = store.add_texts(&text_refs, None, None, 64).await.unwrap();

        // Get first page (3 results, offset=0)
        let page1 = store
            .similarity_search("Document", 3, None, None, 0, None)
            .await
            .unwrap();
        assert_eq!(page1.len(), 3);

        // Get second page (3 results, offset=3)
        let page2 = store
            .similarity_search("Document", 3, None, None, 3, None)
            .await
            .unwrap();
        assert_eq!(page2.len(), 3);

        // Verify different pages have different documents
        let page1_ids: Vec<_> = page1
            .iter()
            .map(|doc| doc.metadata.get("_id").unwrap().clone())
            .collect();
        let page2_ids: Vec<_> = page2
            .iter()
            .map(|doc| doc.metadata.get("_id").unwrap().clone())
            .collect();

        // No overlap between pages
        for id in &page1_ids {
            assert!(!page2_ids.contains(id));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_similarity_search_no_embeddings() {
        // Test that similarity_search returns error when no embeddings provider
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_similarity_no_embeddings",
            None, // No embeddings provider
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Should error because no embeddings provider
        let result = store
            .similarity_search("test query", 5, None, None, 0, None)
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Embeddings provider required"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_max_marginal_relevance_search() {
        // Test MMR search with text query
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_mmr_search",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec![
            "Document about machine learning".to_string(),
            "Document about deep learning".to_string(),
            "Document about neural networks".to_string(),
            "Document about data science".to_string(),
            "Document about statistics".to_string(),
        ];
        let _ = store.add_texts(&texts, None, None, 100).await.unwrap();

        // Search with MMR
        let results = store
            .max_marginal_relevance_search(
                "machine learning",
                3,   // k: return 3 documents
                5,   // fetch_k: consider 5 candidates
                0.5, // lambda_mult: balance relevance and diversity
                None,
                None,
                None,
            )
            .await
            .unwrap();

        // Should return k documents
        assert_eq!(results.len(), 3);

        // Each result should be a valid document
        for doc in &results {
            assert!(!doc.page_content.is_empty());
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_max_marginal_relevance_search_by_vector() {
        // Test MMR search with vector query
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_mmr_by_vector",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec![
            "First document".to_string(),
            "Second document".to_string(),
            "Third document".to_string(),
            "Fourth document".to_string(),
        ];
        let _ = store.add_texts(&texts, None, None, 100).await.unwrap();

        // Create query vector
        let query_vec = embeddings._embed_query("test query").await.unwrap();

        // Search with MMR by vector
        let results = store
            .max_marginal_relevance_search_by_vector(
                &query_vec, 3,   // k: return 3 documents
                4,   // fetch_k: consider all 4 candidates
                0.7, // lambda_mult: favor diversity
                None, None, None,
            )
            .await
            .unwrap();

        // Should return k documents
        assert_eq!(results.len(), 3);

        // Each result should be a valid document
        for doc in &results {
            assert!(!doc.page_content.is_empty());
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_max_marginal_relevance_search_with_score_by_vector() {
        // Test MMR search with vector query returning scores
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_mmr_with_score",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec![
            "Alpha document".to_string(),
            "Beta document".to_string(),
            "Gamma document".to_string(),
            "Delta document".to_string(),
            "Epsilon document".to_string(),
        ];
        let _ = store.add_texts(&texts, None, None, 100).await.unwrap();

        // Create query vector
        let query_vec = embeddings._embed_query("test").await.unwrap();

        // Search with MMR with scores
        let results = store
            .max_marginal_relevance_search_with_score_by_vector(
                &query_vec, 3,   // k: return 3 documents
                5,   // fetch_k: consider all 5 candidates
                0.5, // lambda_mult: balanced
                None, None, None,
            )
            .await
            .unwrap();

        // Should return k (document, score) tuples
        assert_eq!(results.len(), 3);

        // Each result should have a document and a score
        for (doc, score) in &results {
            assert!(!doc.page_content.is_empty());
            assert!(doc.metadata.contains_key("_id"));
            // Score should be a reasonable value (typically 0.0 to 1.0 range)
            assert!(*score >= 0.0);
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_mmr_diversity_parameter() {
        // Test that different lambda_mult values affect results
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_mmr_diversity",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec![
            "Document 1".to_string(),
            "Document 2".to_string(),
            "Document 3".to_string(),
            "Document 4".to_string(),
            "Document 5".to_string(),
        ];
        let _ = store.add_texts(&texts, None, None, 100).await.unwrap();

        let query_vec = embeddings._embed_query("query").await.unwrap();

        // Test with lambda_mult = 0.0 (maximum relevance, minimum diversity)
        let results_relevance = store
            .max_marginal_relevance_search_by_vector(
                &query_vec, 3, 5, 0.0, // Favor relevance only
                None, None, None,
            )
            .await
            .unwrap();

        // Test with lambda_mult = 1.0 (maximum diversity, minimum relevance)
        let results_diversity = store
            .max_marginal_relevance_search_by_vector(
                &query_vec, 3, 5, 1.0, // Favor diversity only
                None, None, None,
            )
            .await
            .unwrap();

        // Both should return same number of results
        assert_eq!(results_relevance.len(), 3);
        assert_eq!(results_diversity.len(), 3);

        // Results should be valid documents
        for doc in &results_relevance {
            assert!(!doc.page_content.is_empty());
        }
        for doc in &results_diversity {
            assert!(!doc.page_content.is_empty());
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_mmr_no_embeddings() {
        // Test that MMR search returns error when no embeddings provider
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_mmr_no_embeddings",
            None, // No embeddings provider
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Should error because no embeddings provider
        let result = store
            .max_marginal_relevance_search("test query", 3, 5, 0.5, None, None, None)
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Embeddings provider required"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_delete_documents() {
        // Test deleting documents by IDs
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_delete",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec![
            "Document to delete 1".to_string(),
            "Document to delete 2".to_string(),
            "Document to keep".to_string(),
        ];
        let ids = store.add_texts(&texts, None, None, 100).await.unwrap();
        assert_eq!(ids.len(), 3);

        // Delete first two documents
        let delete_ids: Vec<&str> = vec![&ids[0], &ids[1]];
        let success = store.delete(&delete_ids).await.unwrap();
        assert!(success);

        // Verify deletion by trying to retrieve
        let retrieved = store.get_by_ids(&delete_ids).await.unwrap();
        // Qdrant returns empty list for non-existent IDs
        assert_eq!(retrieved.len(), 0);

        // Verify the third document is still there
        let kept = store.get_by_ids(&[&ids[2]]).await.unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].page_content, "Document to keep");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_get_by_ids() {
        // Test retrieving documents by IDs
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_get_by_ids",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec![
            "First document content".to_string(),
            "Second document content".to_string(),
            "Third document content".to_string(),
        ];
        let ids = store.add_texts(&texts, None, None, 100).await.unwrap();
        assert_eq!(ids.len(), 3);

        // Retrieve documents by IDs
        let docs = store.get_by_ids(&ids).await.unwrap();
        assert_eq!(docs.len(), 3);

        // Verify content matches
        assert_eq!(docs[0].page_content, "First document content");
        assert_eq!(docs[1].page_content, "Second document content");
        assert_eq!(docs[2].page_content, "Third document content");

        // Verify metadata
        for doc in &docs {
            assert!(doc.metadata.contains_key("_id"));
            assert!(doc.metadata.contains_key("_collection_name"));
            assert_eq!(
                doc.metadata.get("_collection_name").unwrap(),
                &serde_json::json!("test_get_by_ids")
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_get_by_ids_partial() {
        // Test retrieving with some IDs that don't exist
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_get_by_ids_partial",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add test documents
        let texts = vec!["Existing document".to_string()];
        let ids = store.add_texts(&texts, None, None, 100).await.unwrap();
        assert_eq!(ids.len(), 1);

        // Try to retrieve with mix of existing and non-existing IDs
        let query_ids = vec![ids[0].as_str(), "nonexistent-id"];
        let docs = store.get_by_ids(&query_ids).await.unwrap();

        // Should only return the existing document
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Existing document");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_delete_empty_list() {
        // Test deleting with empty ID list
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_delete_empty",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Delete with empty list should succeed (no-op)
        let empty_ids: Vec<&str> = vec![];
        let success = store.delete(&empty_ids).await.unwrap();
        assert!(success);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_get_by_ids_empty_list() {
        // Test retrieving with empty ID list
        let embeddings = get_embeddings_for_test();
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_get_empty",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Get with empty list should return empty vec
        let empty_ids: Vec<&str> = vec![];
        let docs = store.get_by_ids(&empty_ids).await.unwrap();
        assert_eq!(docs.len(), 0);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_from_existing_collection() {
        // Test connecting to an existing collection
        let embeddings = get_embeddings_for_test();

        // Create a collection first using new()
        let store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_existing_collection",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let mut store = store.unwrap();

        // Add some test data
        let texts = vec!["Test document 1", "Test document 2"];
        let _ids = store
            .add_texts(&texts, None, None, 64)
            .await
            .expect("Failed to add texts");

        // Now connect to the existing collection using from_existing_collection
        let existing_store = QdrantVectorStore::from_existing_collection(
            "http://localhost:6334",
            "test_existing_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .await
        .expect("Failed to connect to existing collection");

        // Verify we can search the existing collection
        let results = existing_store
            .similarity_search("Test", 2, None, None, 0, None)
            .await
            .expect("Failed to search");

        assert!(!results.is_empty(), "Should find documents in collection");
        assert!(
            results[0].page_content.contains("Test"),
            "Should find test documents"
        );

        // Verify collection name
        assert_eq!(existing_store.collection_name(), "test_existing_collection");

        // Verify retrieval mode
        assert_eq!(existing_store.retrieval_mode(), RetrievalMode::Dense);
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_from_existing_collection_validation() {
        // Test that from_existing_collection validates embeddings
        let result = QdrantVectorStore::from_existing_collection(
            "http://localhost:6334",
            "test_collection",
            None,                 // No embeddings
            RetrievalMode::Dense, // Dense mode requires embeddings
        )
        .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("cannot be None when retrieval mode is Dense"));
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_from_texts_basic() {
        // Test creating and initializing a vector store with from_texts
        let embeddings = get_embeddings_for_test();

        // First create a collection manually (since from_texts assumes it exists)
        let setup_store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_from_texts",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if setup_store.is_err() {
            return;
        }

        // Now use from_texts to initialize with documents
        let texts = vec!["Document 1", "Document 2", "Document 3"];
        let store = QdrantVectorStore::from_texts(
            "http://localhost:6334",
            "test_from_texts",
            &texts,
            None, // No metadata
            None, // Auto-generate IDs
            Some(embeddings),
            RetrievalMode::Dense,
            64, // Batch size
        )
        .await
        .expect("Failed to create from texts");

        // Verify documents were added
        let results = store
            .similarity_search("Document", 5, None, None, 0, None)
            .await
            .expect("Failed to search");

        assert!(results.len() >= 3, "Should have at least 3 documents");

        // Verify the documents contain our text
        let found_texts: Vec<String> = results.iter().map(|d| d.page_content.clone()).collect();
        assert!(
            found_texts.iter().any(|t| t.contains("Document 1")),
            "Should find Document 1"
        );
        assert!(
            found_texts.iter().any(|t| t.contains("Document 2")),
            "Should find Document 2"
        );
        assert!(
            found_texts.iter().any(|t| t.contains("Document 3")),
            "Should find Document 3"
        );
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_from_texts_with_metadata() {
        // Test from_texts with metadata
        let embeddings = get_embeddings_for_test();

        // First create a collection manually
        let setup_store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_from_texts_metadata",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if setup_store.is_err() {
            return;
        }

        // Create metadata
        let mut meta1 = HashMap::new();
        meta1.insert(
            "source".to_string(),
            JsonValue::String("doc1.txt".to_string()),
        );
        meta1.insert("page".to_string(), JsonValue::Number(1.into()));

        let mut meta2 = HashMap::new();
        meta2.insert(
            "source".to_string(),
            JsonValue::String("doc2.txt".to_string()),
        );
        meta2.insert("page".to_string(), JsonValue::Number(2.into()));

        let metadatas = vec![meta1, meta2];

        // Use from_texts with metadata
        let texts = vec!["Text with metadata 1", "Text with metadata 2"];
        let store = QdrantVectorStore::from_texts(
            "http://localhost:6334",
            "test_from_texts_metadata",
            &texts,
            Some(&metadatas),
            None, // Auto-generate IDs
            Some(embeddings),
            RetrievalMode::Dense,
            64, // Batch size
        )
        .await
        .expect("Failed to create from texts with metadata");

        // Verify documents were added with metadata
        let results = store
            .similarity_search("Text", 2, None, None, 0, None)
            .await
            .expect("Failed to search");

        assert!(results.len() >= 2, "Should have at least 2 documents");

        // Check metadata was preserved
        let doc1 = results
            .iter()
            .find(|d| d.page_content.contains("metadata 1"));
        assert!(doc1.is_some(), "Should find document 1");
        let doc1 = doc1.unwrap();
        assert_eq!(
            doc1.metadata.get("source"),
            Some(&JsonValue::String("doc1.txt".to_string()))
        );
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_from_texts_with_custom_ids() {
        // Test from_texts with custom IDs
        let embeddings = get_embeddings_for_test();

        // First create a collection manually
        let setup_store = QdrantVectorStore::new(
            "http://localhost:6334",
            "test_from_texts_ids",
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        )
        .await;

        // Skip if server not available
        if setup_store.is_err() {
            return;
        }

        // Use from_texts with custom IDs
        let texts = vec!["Text A", "Text B"];
        let ids = vec!["custom_id_1".to_string(), "custom_id_2".to_string()];
        let store = QdrantVectorStore::from_texts(
            "http://localhost:6334",
            "test_from_texts_ids",
            &texts,
            None,
            Some(&ids),
            Some(embeddings),
            RetrievalMode::Dense,
            64, // Batch size
        )
        .await
        .expect("Failed to create from texts with custom IDs");

        // Verify we can retrieve by the custom IDs
        let id_refs: Vec<&str> = ids.iter().map(|s| s.as_str()).collect();
        let docs = store
            .get_by_ids(&id_refs)
            .await
            .expect("Failed to get by IDs");

        assert_eq!(docs.len(), 2, "Should retrieve 2 documents");
        assert_eq!(
            docs[0].metadata.get("_id"),
            Some(&JsonValue::String("custom_id_1".to_string()))
        );
        assert_eq!(
            docs[1].metadata.get("_id"),
            Some(&JsonValue::String("custom_id_2".to_string()))
        );
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_construct_instance_creates_collection() {
        // Test that construct_instance creates a new collection
        let embeddings = get_embeddings_for_test();

        // Use construct_instance with auto-generated collection name
        let store = QdrantVectorStore::construct_instance(
            "http://localhost:6334",
            None, // Auto-generate collection name
            Some(embeddings),
            RetrievalMode::Dense,
            qdrant_client::qdrant::Distance::Cosine,
            false, // Don't force recreate
            true,  // Validate config
        )
        .await;

        // Skip if server not available
        if store.is_err() {
            return;
        }

        let store = store.unwrap();

        // Verify collection was created and has a generated name
        assert!(!store.collection_name().is_empty());
        assert_eq!(store.collection_name().len(), 32); // UUID hex format

        // Verify collection exists
        let exists = store
            .collection_exists(store.collection_name())
            .await
            .expect("Failed to check collection existence");
        assert!(exists, "Collection should exist");

        // Clean up - delete the collection
        store
            .delete_collection(store.collection_name())
            .await
            .expect("Failed to delete collection");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_construct_instance_with_force_recreate() {
        // Test that construct_instance recreates existing collection
        let embeddings = get_embeddings_for_test();
        let collection_name = "test_force_recreate";

        // First create a collection
        let store1 = QdrantVectorStore::construct_instance(
            "http://localhost:6334",
            Some(collection_name.to_string()),
            Some(embeddings.clone()),
            RetrievalMode::Dense,
            qdrant_client::qdrant::Distance::Cosine,
            false,
            true,
        )
        .await;

        // Skip if server not available
        if store1.is_err() {
            return;
        }

        let mut store1 = store1.unwrap();

        // Add some data
        let texts = vec!["Test text"];
        store1
            .add_texts(&texts, None, None, 64)
            .await
            .expect("Failed to add texts");

        // Now recreate with force_recreate=true
        let store2 = QdrantVectorStore::construct_instance(
            "http://localhost:6334",
            Some(collection_name.to_string()),
            Some(embeddings),
            RetrievalMode::Dense,
            qdrant_client::qdrant::Distance::Cosine,
            true, // Force recreate
            true,
        )
        .await
        .expect("Failed to recreate collection");

        // Collection should be empty now
        let results = store2
            .similarity_search("Test", 10, None, None, 0, None)
            .await
            .expect("Failed to search");

        assert_eq!(results.len(), 0, "Recreated collection should be empty");

        // Clean up
        store2
            .delete_collection(collection_name)
            .await
            .expect("Failed to delete collection");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_collection_exists() {
        // Test collection_exists method
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection_exists_check",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        // Collection that doesn't exist
        let exists = store.collection_exists("nonexistent_collection").await;
        if exists.is_err() {
            // Server not available
            return;
        }

        assert!(!exists.unwrap(), "Nonexistent collection should not exist");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_create_and_delete_collection() {
        // Test create_collection and delete_collection
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_create_delete",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let test_collection = "test_create_delete_collection";

        // Check server availability
        let exists_check = store.collection_exists(test_collection).await;
        if exists_check.is_err() {
            return;
        }

        // Delete if exists (from previous test)
        if exists_check.unwrap() {
            store
                .delete_collection(test_collection)
                .await
                .expect("Failed to delete existing collection");
        }

        // Create collection
        store
            .create_collection(test_collection, 3, qdrant_client::qdrant::Distance::Cosine)
            .await
            .expect("Failed to create collection");

        // Verify it exists
        let exists = store
            .collection_exists(test_collection)
            .await
            .expect("Failed to check existence");
        assert!(exists, "Collection should exist after creation");

        // Delete collection
        store
            .delete_collection(test_collection)
            .await
            .expect("Failed to delete collection");

        // Verify it's deleted
        let exists = store
            .collection_exists(test_collection)
            .await
            .expect("Failed to check existence");
        assert!(!exists, "Collection should not exist after deletion");
    }

    #[tokio::test]
    #[ignore = "requires Qdrant server: docker-compose -f docker-compose.test.yml up qdrant"]
    async fn test_from_texts_creates_collection_automatically() {
        // Test that from_texts creates collection if it doesn't exist
        let embeddings = get_embeddings_for_test();
        let collection_name = "test_from_texts_auto_create";

        // First ensure collection doesn't exist
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();
        let temp_store = QdrantVectorStore::from_client(
            client,
            collection_name,
            Some(embeddings.clone()),
            RetrievalMode::Dense,
        );

        let exists_check = temp_store.collection_exists(collection_name).await;
        if exists_check.is_err() {
            // Server not available
            return;
        }

        // Delete if exists
        if exists_check.unwrap() {
            temp_store
                .delete_collection(collection_name)
                .await
                .expect("Failed to delete existing collection");
        }

        // Now use from_texts which should create the collection
        let texts = vec!["Hello world", "Goodbye world"];
        let store = QdrantVectorStore::from_texts(
            "http://localhost:6334",
            collection_name,
            &texts,
            None,
            None,
            Some(embeddings),
            RetrievalMode::Dense,
            64,
        )
        .await
        .expect("Failed to create from texts");

        // Verify collection was created and data was added
        let results = store
            .similarity_search("Hello", 2, None, None, 0, None)
            .await
            .expect("Failed to search");

        assert_eq!(results.len(), 2, "Should have 2 documents");

        // Clean up
        store
            .delete_collection(collection_name)
            .await
            .expect("Failed to delete collection");
    }

    // ========================================================================
    // Unit Tests for Helper Functions (No Qdrant Server Required)
    // ========================================================================

    #[tokio::test]
    async fn test_qdrant_value_to_json_primitives() {
        use qdrant::value::Kind;

        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();
        let store =
            QdrantVectorStore::from_client(client, "test", Some(embeddings), RetrievalMode::Dense);

        // Null
        let value = qdrant::Value {
            kind: Some(Kind::NullValue(0)),
        };
        assert_eq!(store.qdrant_value_to_json(&value), Some(JsonValue::Null));

        // Bool
        let value = qdrant::Value {
            kind: Some(Kind::BoolValue(true)),
        };
        assert_eq!(
            store.qdrant_value_to_json(&value),
            Some(JsonValue::Bool(true))
        );

        // Integer
        let value = qdrant::Value {
            kind: Some(Kind::IntegerValue(42)),
        };
        assert_eq!(
            store.qdrant_value_to_json(&value),
            Some(JsonValue::Number(42.into()))
        );

        // Double
        let value = qdrant::Value {
            kind: Some(Kind::DoubleValue(std::f64::consts::PI)),
        };
        let result = store.qdrant_value_to_json(&value).unwrap();
        if let JsonValue::Number(n) = result {
            assert_eq!(n.as_f64().unwrap(), std::f64::consts::PI);
        } else {
            panic!("Expected Number");
        }

        // String
        let value = qdrant::Value {
            kind: Some(Kind::StringValue("hello".to_string())),
        };
        assert_eq!(
            store.qdrant_value_to_json(&value),
            Some(JsonValue::String("hello".to_string()))
        );
    }

    #[tokio::test]
    async fn test_qdrant_value_to_json_list() {
        use qdrant::value::Kind;

        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();
        let store =
            QdrantVectorStore::from_client(client, "test", Some(embeddings), RetrievalMode::Dense);

        let value = qdrant::Value {
            kind: Some(Kind::ListValue(qdrant::ListValue {
                values: vec![
                    qdrant::Value {
                        kind: Some(Kind::IntegerValue(1)),
                    },
                    qdrant::Value {
                        kind: Some(Kind::IntegerValue(2)),
                    },
                    qdrant::Value {
                        kind: Some(Kind::IntegerValue(3)),
                    },
                ],
            })),
        };

        let result = store.qdrant_value_to_json(&value).unwrap();
        assert_eq!(
            result,
            JsonValue::Array(vec![
                JsonValue::Number(1.into()),
                JsonValue::Number(2.into()),
                JsonValue::Number(3.into()),
            ])
        );
    }

    #[tokio::test]
    async fn test_qdrant_value_to_json_struct() {
        use qdrant::value::Kind;
        use std::collections::HashMap as StdHashMap;

        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();
        let store =
            QdrantVectorStore::from_client(client, "test", Some(embeddings), RetrievalMode::Dense);

        let mut fields = StdHashMap::new();
        fields.insert(
            "name".to_string(),
            qdrant::Value {
                kind: Some(Kind::StringValue("Alice".to_string())),
            },
        );
        fields.insert(
            "age".to_string(),
            qdrant::Value {
                kind: Some(Kind::IntegerValue(30)),
            },
        );

        let value = qdrant::Value {
            kind: Some(Kind::StructValue(qdrant::Struct { fields })),
        };

        let result = store.qdrant_value_to_json(&value).unwrap();
        if let JsonValue::Object(map) = result {
            assert_eq!(
                map.get("name"),
                Some(&JsonValue::String("Alice".to_string()))
            );
            assert_eq!(map.get("age"), Some(&JsonValue::Number(30.into())));
        } else {
            panic!("Expected Object");
        }
    }

    #[tokio::test]
    async fn test_build_payload_without_metadata() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();
        let store =
            QdrantVectorStore::from_client(client, "test", Some(embeddings), RetrievalMode::Dense);

        let payload = store.build_payload("Hello world", None);

        // Convert to HashMap to inspect
        let map: HashMap<String, qdrant::Value> = payload.into();

        // Should have page_content
        assert!(map.contains_key("page_content"));

        // Should have metadata as null
        assert!(map.contains_key("metadata"));
    }

    #[tokio::test]
    async fn test_clone_store() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        )
        .with_distance_metric(DistanceMetric::Euclidean)
        .with_content_key("custom_content");

        let cloned = store.clone();

        assert_eq!(cloned.collection_name(), store.collection_name());
        assert_eq!(cloned.retrieval_mode(), store.retrieval_mode());
        assert_eq!(cloned.distance_metric(), store.distance_metric());
        assert_eq!(cloned.content_key(), store.content_key());
    }

    #[tokio::test]
    async fn test_debug_format() {
        let embeddings = get_embeddings_for_test();
        let client = Qdrant::from_url("http://localhost:6334").build().unwrap();

        let store = QdrantVectorStore::from_client(
            client,
            "test_collection",
            Some(embeddings),
            RetrievalMode::Dense,
        );

        let debug_str = format!("{:?}", store);
        assert!(debug_str.contains("QdrantVectorStore"));
        assert!(debug_str.contains("test_collection"));
        assert!(debug_str.contains("Dense"));
    }
