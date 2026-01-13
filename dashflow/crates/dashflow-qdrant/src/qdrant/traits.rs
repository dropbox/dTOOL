use super::*;

/// Implementation of Retriever trait for `QdrantVectorStore`
///
/// Enables the Qdrant vector store to be used as a retriever in chains and workflows.
#[async_trait::async_trait]
impl Retriever for QdrantVectorStore {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        // Default to k=4 (standard retriever behavior)
        self.similarity_search(query, 4, None, None, 0, None).await
    }

    fn name(&self) -> String {
        "QdrantVectorStore".to_string()
    }
}

/// Implementation of `DocumentIndex` trait for `QdrantVectorStore`
///
/// This enables the Qdrant vector store to be used with the document indexing API,
/// providing intelligent change detection and cleanup of outdated documents.
#[async_trait::async_trait]
impl DocumentIndex for QdrantVectorStore {
    async fn upsert(
        &self,
        items: &[Document],
    ) -> std::result::Result<UpsertResponse, Box<dyn std::error::Error + Send + Sync>> {
        if items.is_empty() {
            return Ok(UpsertResponse::all_succeeded(vec![]));
        }

        // Extract IDs from documents
        let ids: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(i, doc)| doc.id.clone().unwrap_or_else(|| format!("doc_{i}")))
            .collect();

        // Convert documents to texts and metadata
        let texts: Vec<String> = items.iter().map(|doc| doc.page_content.clone()).collect();
        let metadatas: Vec<HashMap<String, JsonValue>> =
            items.iter().map(|doc| doc.metadata.clone()).collect();

        // Call add_texts with the extracted data
        // This will embed the texts and upsert them to Qdrant
        // Use default batch size of 64
        match self
            .add_texts_internal(&texts, Some(&metadatas), Some(&ids))
            .await
        {
            Ok(_) => Ok(UpsertResponse::all_succeeded(ids)),
            Err(_e) => {
                // If there's an error, mark all as failed
                Ok(UpsertResponse::all_failed(ids))
            }
        }
    }

    async fn delete(
        &self,
        ids: Option<&[String]>,
    ) -> std::result::Result<DeleteResponse, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ids) = ids {
            if ids.is_empty() {
                return Ok(DeleteResponse::with_count(0));
            }

            let ids_refs: Vec<&str> = ids.iter().map(std::string::String::as_str).collect();
            match self.delete(&ids_refs).await {
                Ok(_) => Ok(DeleteResponse::with_count(ids.len())),
                Err(e) => Err(Box::new(e)),
            }
        } else {
            // Qdrant requires explicit IDs for deletion
            Err(Box::new(Error::InvalidInput(
                "Qdrant requires explicit IDs for deletion".to_string(),
            )))
        }
    }

    async fn get(
        &self,
        ids: &[String],
    ) -> std::result::Result<Vec<Document>, Box<dyn std::error::Error + Send + Sync>> {
        let ids_refs: Vec<&str> = ids.iter().map(std::string::String::as_str).collect();
        self.get_by_ids(&ids_refs)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }
}

pub(super) fn hashmap_to_qdrant_filter(filter: &HashMap<String, JsonValue>) -> Option<Filter> {
    if filter.is_empty() {
        return None;
    }

    let mut must_conditions = Vec::new();

    for (key, value) in filter {
        let field_key = format!("metadata.{key}");

        let match_value = match value {
            JsonValue::String(s) => Some(qdrant_client::qdrant::r#match::MatchValue::Keyword(
                s.clone(),
            )),
            #[allow(clippy::manual_map)] // Nested cast_possible_truncation allow requires this structure
            JsonValue::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(qdrant_client::qdrant::r#match::MatchValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    #[allow(clippy::cast_possible_truncation)] // Intentional: JSON floats to Qdrant integer match
                    Some(qdrant_client::qdrant::r#match::MatchValue::Integer(f as i64))
                } else {
                    None
                }
            }
            JsonValue::Bool(b) => Some(qdrant_client::qdrant::r#match::MatchValue::Boolean(*b)),
            JsonValue::Null | JsonValue::Array(_) | JsonValue::Object(_) => None,
        };

        if let Some(mv) = match_value {
            must_conditions.push(Condition {
                condition_one_of: Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(
                    FieldCondition {
                        key: field_key,
                        r#match: Some(Match {
                            match_value: Some(mv),
                        }),
                        ..Default::default()
                    },
                )),
            });
        }
    }

    if must_conditions.is_empty() {
        return None;
    }

    Some(Filter {
        must: must_conditions,
        ..Default::default()
    })
}

#[async_trait::async_trait]
impl dashflow::core::vector_stores::VectorStore for QdrantVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        self.embeddings.clone()
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.distance_metric
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let text_strings: Vec<&str> = texts.iter().map(std::convert::AsRef::as_ref).collect();
        self.add_texts(&text_strings, metadatas, ids, 64).await
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        match ids {
            Some(ids) => QdrantVectorStore::delete(self, ids).await,
            None => Err(Error::NotImplemented(
                "delete all documents not supported for Qdrant".to_string(),
            )),
        }
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        QdrantVectorStore::get_by_ids(self, ids).await
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let qdrant_filter = filter.and_then(hashmap_to_qdrant_filter);
        QdrantVectorStore::similarity_search(self, query, k, qdrant_filter, None, 0, None).await
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<(Document, f32)>> {
        let qdrant_filter = filter.and_then(hashmap_to_qdrant_filter);
        QdrantVectorStore::similarity_search_with_score(
            self,
            query,
            k,
            qdrant_filter,
            None,
            0,
            None,
        )
        .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, JsonValue>>,
    ) -> Result<Vec<Document>> {
        let qdrant_filter = filter.and_then(hashmap_to_qdrant_filter);
        QdrantVectorStore::similarity_search_by_vector(
            self,
            embedding,
            k,
            qdrant_filter,
            None,
            0,
            None,
        )
        .await
    }
}

impl QdrantVectorStore {
    async fn add_texts_internal(
        &self,
        texts: &[String],
        metadatas: Option<&[HashMap<String, JsonValue>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        let text_count = texts.len();
        if text_count == 0 {
            return Ok(vec![]);
        }

        // Validate metadata count matches text count
        if let Some(metadatas) = metadatas {
            if metadatas.len() != text_count {
                return Err(Error::InvalidInput(format!(
                    "Metadatas count ({}) does not match texts count ({})",
                    metadatas.len(),
                    text_count
                )));
            }
        }

        // Generate or validate IDs
        let point_ids = if let Some(ids) = ids {
            if ids.len() != text_count {
                return Err(Error::InvalidInput(format!(
                    "IDs count ({}) does not match texts count ({})",
                    ids.len(),
                    text_count
                )));
            }
            ids.to_vec()
        } else {
            (0..text_count)
                .map(|_| uuid::Uuid::new_v4().simple().to_string())
                .collect()
        };

        let batch_vectors = self.build_vectors(texts).await?;

        let mut batch_payloads: Vec<Payload> = Vec::with_capacity(text_count);
        for (i, text) in texts.iter().enumerate() {
            let metadata = metadatas.and_then(|metas| metas.get(i));
            batch_payloads.push(self.build_payload(text, metadata));
        }

        let batch_points: Vec<qdrant::PointStruct> = point_ids
            .iter()
            .zip(batch_vectors.iter())
            .zip(batch_payloads.iter())
            .map(|((id, vector), payload)| qdrant::PointStruct {
                id: Some(qdrant::PointId::from(id.as_str())),
                vectors: Some(vector.clone()),
                payload: payload.clone().into(),
            })
            .collect();

        let upsert_request = UpsertPointsBuilder::new(&self.collection_name, batch_points);
        self.client
            .upsert_points(upsert_request)
            .await
            .map_err(|e| Error::other(format!("Failed to upsert to Qdrant: {e}")))?;

        Ok(point_ids)
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_hashmap_to_qdrant_filter_empty_returns_none() {
        let filter: HashMap<String, JsonValue> = HashMap::new();
        assert!(hashmap_to_qdrant_filter(&filter).is_none());
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_string_value() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("documents"));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        // Verify the field key is prefixed with metadata.
        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(field)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(field.key, "metadata.category");
            // Verify it's a keyword match
            assert!(field.r#match.is_some());
        } else {
            panic!("Expected field condition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_integer_value() {
        let mut filter = HashMap::new();
        filter.insert("count".to_string(), json!(42));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(field)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(field.key, "metadata.count");
            assert!(field.r#match.is_some());
        } else {
            panic!("Expected field condition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_float_truncates_to_integer() {
        let mut filter = HashMap::new();
        filter.insert("score".to_string(), json!(std::f64::consts::PI));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(field)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(field.key, "metadata.score");
            // Float PI should be truncated to integer 3
            if let Some(Match {
                match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Integer(i)),
            }) = &field.r#match
            {
                assert_eq!(*i, 3);
            } else {
                panic!("Expected integer match");
            }
        } else {
            panic!("Expected field condition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_boolean_value() {
        let mut filter = HashMap::new();
        filter.insert("is_active".to_string(), json!(true));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        assert_eq!(qdrant_filter.must.len(), 1);

        if let Some(qdrant_client::qdrant::condition::ConditionOneOf::Field(field)) =
            &qdrant_filter.must[0].condition_one_of
        {
            assert_eq!(field.key, "metadata.is_active");
            if let Some(Match {
                match_value: Some(qdrant_client::qdrant::r#match::MatchValue::Boolean(b)),
            }) = &field.r#match
            {
                assert!(*b);
            } else {
                panic!("Expected boolean match");
            }
        } else {
            panic!("Expected field condition");
        }
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_null_value_ignored() {
        let mut filter = HashMap::new();
        filter.insert("null_field".to_string(), JsonValue::Null);

        // Null values are not converted to conditions
        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_none());
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_array_value_ignored() {
        let mut filter = HashMap::new();
        filter.insert("tags".to_string(), json!(["a", "b", "c"]));

        // Array values are not converted to conditions
        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_none());
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_object_value_ignored() {
        let mut filter = HashMap::new();
        filter.insert("nested".to_string(), json!({"key": "value"}));

        // Object values are not converted to conditions
        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_none());
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_multiple_valid_values() {
        let mut filter = HashMap::new();
        filter.insert("category".to_string(), json!("docs"));
        filter.insert("count".to_string(), json!(10));
        filter.insert("active".to_string(), json!(true));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        // All three should be in must conditions
        assert_eq!(qdrant_filter.must.len(), 3);
    }

    #[test]
    fn test_hashmap_to_qdrant_filter_mixed_valid_invalid_values() {
        let mut filter = HashMap::new();
        filter.insert("valid_string".to_string(), json!("test"));
        filter.insert("invalid_null".to_string(), JsonValue::Null);
        filter.insert("invalid_array".to_string(), json!([1, 2, 3]));

        let result = hashmap_to_qdrant_filter(&filter);
        assert!(result.is_some());

        let qdrant_filter = result.unwrap();
        // Only the valid string should be in must conditions
        assert_eq!(qdrant_filter.must.len(), 1);
    }
}
