use super::*;

impl QdrantVectorStore {
    /// Checks if a collection exists in Qdrant.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection to check
    ///
    /// # Returns
    ///
    /// Returns `Ok(true)` if the collection exists, `Ok(false)` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if the Qdrant API request fails.
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `client.collection_exists(collection_name)` in Python baseline
    /// at `dashflow_qdrant/qdrant.py:923`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// let exists = store.collection_exists("my_collection").await?;
    /// if exists {
    ///     println!("Collection exists");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn collection_exists(&self, collection_name: &str) -> Result<bool> {
        // Python: client.collection_exists(collection_name)
        self.client
            .collection_exists(collection_name)
            .await
            .map_err(|e| Error::Other(format!("Failed to check if collection exists: {e}")))
    }

    /// Deletes a collection from Qdrant.
    ///
    /// **Warning**: This operation is destructive and cannot be undone. All data in
    /// the collection will be permanently deleted.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection to delete
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The collection does not exist
    /// - The Qdrant API request fails
    /// - Permission denied
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `client.delete_collection(collection_name)` in Python baseline
    /// at `dashflow_qdrant/qdrant.py:926`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// store.delete_collection("old_collection").await?;
    /// println!("Collection deleted");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn delete_collection(&self, collection_name: &str) -> Result<()> {
        // Python: client.delete_collection(collection_name)
        self.client
            .delete_collection(collection_name)
            .await
            .map_err(|e| Error::Other(format!("Failed to delete collection: {e}")))?;
        Ok(())
    }

    /// Creates a new collection with the specified configuration.
    ///
    /// This is a low-level method for creating collections with custom vector
    /// configurations. For most use cases, prefer using [`construct_instance()`](Self::construct_instance)
    /// which handles configuration automatically.
    ///
    /// # Arguments
    ///
    /// * `collection_name` - Name of the collection to create
    /// * `vector_size` - Dimension of the dense vectors
    /// * `distance` - Distance metric for similarity calculations
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on success.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Collection already exists
    /// - Invalid vector size or distance metric
    /// - Qdrant API request fails
    ///
    /// # Python Baseline Compatibility
    ///
    /// Matches `client.create_collection(**collection_create_options)` in Python baseline
    /// at `dashflow_qdrant/qdrant.py:982`.
    ///
    /// Python creates collections with complex vector configurations:
    /// - Dense vectors: `vectors_config = {vector_name: VectorParams(size=..., distance=...)}`
    /// - Sparse vectors: `sparse_vectors_config = {sparse_vector_name: SparseVectorParams(...)}`
    ///
    /// This Rust implementation currently only supports dense vectors.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # use dashflow_qdrant::QdrantVectorStore;
    /// # use qdrant_client::qdrant::Distance;
    /// # async fn example(store: &QdrantVectorStore) -> Result<(), Box<dyn std::error::Error>> {
    /// store.create_collection("my_collection", 384, Distance::Cosine).await?;
    /// println!("Collection created");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_collection(
        &self,
        collection_name: &str,
        vector_size: u64,
        distance: Distance,
    ) -> Result<()> {
        // Python: client.create_collection(**collection_create_options)
        // Python creates: vectors_config = {vector_name: VectorParams(size=..., distance=...)}

        use qdrant_client::qdrant::{
            CreateCollectionBuilder, VectorParams, VectorParamsBuilder, VectorsConfig,
        };

        // Build vector params for dense vectors
        // Python: models.VectorParams(size=vector_size, distance=distance)
        let vector_params: VectorParams = VectorParamsBuilder::new(vector_size, distance).build();

        // Create collection with vector configuration
        // Python uses vector_name as key (default: "" for unnamed vector)
        // Rust qdrant-client: unnamed vector uses params directly, named vectors use params_map
        let vectors_config = if self.vector_name.is_empty() {
            // Unnamed vector (default vector)
            VectorsConfig {
                config: Some(qdrant_client::qdrant::vectors_config::Config::Params(
                    vector_params,
                )),
            }
        } else {
            // Named vector - use a map of vector names to params
            let mut vector_map = std::collections::HashMap::new();
            vector_map.insert(self.vector_name.clone(), vector_params);
            VectorsConfig {
                config: Some(qdrant_client::qdrant::vectors_config::Config::ParamsMap(
                    qdrant_client::qdrant::VectorParamsMap { map: vector_map },
                )),
            }
        };

        let create_collection =
            CreateCollectionBuilder::new(collection_name).vectors_config(vectors_config);

        self.client
            .create_collection(create_collection)
            .await
            .map_err(|e| Error::Other(format!("Failed to create collection: {e}")))?;

        Ok(())
    }

}
