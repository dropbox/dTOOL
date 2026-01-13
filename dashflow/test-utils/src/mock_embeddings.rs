//! Mock embeddings provider for testing.
//!
//! This module provides a simple, deterministic embeddings implementation
//! that doesn't require external API keys or network calls. Perfect for
//! unit testing vector stores and other components that need embeddings.

use async_trait::async_trait;
use dashflow::core::embeddings::Embeddings;
use dashflow::core::error::Result;

/// Simple mock embeddings provider for testing.
///
/// Generates deterministic 3-dimensional vectors based on text content.
/// The vectors are normalized and predictable, making them perfect for
/// unit tests that need embeddings but don't care about semantic meaning.
///
/// # Vector Generation
///
/// For each text:
/// - x component: normalized first byte value (or 0 if empty)
/// - y component: normalized second byte value (or 0 if too short)
/// - z component: normalized text length
///
/// The resulting vector is then normalized to unit length.
///
/// # Examples
///
/// ```rust
/// use dashflow_test_utils::MockEmbeddings;
/// use dashflow::core::embeddings::Embeddings;
/// use dashflow::embed;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new());
///
///     let texts = vec!["Hello".to_string(), "World".to_string()];
///     let vectors = embed(embeddings, &texts).await?;
///
///     assert_eq!(vectors.len(), 2);
///     assert_eq!(vectors[0].len(), 3); // 3D vectors
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct MockEmbeddings {
    /// Dimensionality of generated vectors (default: 3)
    pub dimensions: usize,
}

impl MockEmbeddings {
    /// Creates a new mock embeddings provider with 3-dimensional vectors.
    #[must_use]
    pub fn new() -> Self {
        Self { dimensions: 3 }
    }

    /// Creates a new mock embeddings provider with custom dimensionality.
    ///
    /// # Arguments
    ///
    /// * `dimensions` - Number of dimensions for generated vectors
    ///
    /// # Examples
    ///
    /// ```rust
    /// use dashflow_test_utils::MockEmbeddings;
    ///
    /// let embeddings = MockEmbeddings::with_dimensions(128);
    /// ```
    #[must_use]
    pub fn with_dimensions(dimensions: usize) -> Self {
        Self { dimensions }
    }

    /// Generates a deterministic vector for a single text.
    ///
    /// The vector is based on the text content and is normalized to unit length.
    fn generate_vector(&self, text: &str) -> Vec<f32> {
        let bytes = text.as_bytes();

        // Generate base components from text
        let x = if bytes.is_empty() {
            0.0
        } else {
            f32::from(bytes[0]) / 255.0
        };

        let y = if bytes.len() < 2 {
            0.0
        } else {
            f32::from(bytes[1]) / 255.0
        };

        let z = (text.len() as f32 / 100.0).min(1.0);

        // For higher dimensions, generate additional components based on text hash
        let mut vector = vec![x, y, z];

        if self.dimensions > 3 {
            // Use simple hash-based generation for additional dimensions
            for i in 3..self.dimensions {
                let byte_index = i % bytes.len().max(1);
                let byte_val = if byte_index < bytes.len() {
                    bytes[byte_index]
                } else {
                    (i as u8).wrapping_mul(37)
                };
                vector.push(f32::midpoint(
                    f32::from(byte_val) / 255.0,
                    i as f32 / self.dimensions as f32,
                ));
            }
        } else if self.dimensions < 3 {
            vector.truncate(self.dimensions);
        }

        // Normalize to unit length
        let magnitude: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            vector.iter().map(|v| v / magnitude).collect()
        } else {
            // If all zero, return equal components
            vec![1.0 / (self.dimensions as f32).sqrt(); self.dimensions]
        }
    }
}

#[async_trait]
impl Embeddings for MockEmbeddings {
    async fn _embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(texts
            .iter()
            .map(|text| self.generate_vector(text))
            .collect())
    }

    async fn _embed_query(&self, text: &str) -> Result<Vec<f32>> {
        Ok(self.generate_vector(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::{embed, embed_query};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_mock_embeddings_basic() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new());

        let texts = vec!["Hello".to_string(), "World".to_string()];
        let vectors = embed(embeddings, &texts).await.unwrap();

        assert_eq!(vectors.len(), 2);
        assert_eq!(vectors[0].len(), 3);
        assert_eq!(vectors[1].len(), 3);

        // Vectors should be normalized (magnitude ≈ 1.0)
        for vector in &vectors {
            let magnitude: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
            assert!((magnitude - 1.0).abs() < 0.001);
        }
    }

    #[tokio::test]
    async fn test_mock_embeddings_deterministic() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new());

        let text = "Test".to_string();
        let vector1 = embed(Arc::clone(&embeddings), std::slice::from_ref(&text))
            .await
            .unwrap();
        let vector2 = embed(embeddings, std::slice::from_ref(&text))
            .await
            .unwrap();

        // Should generate same vector for same text
        assert_eq!(vector1, vector2);
    }

    #[tokio::test]
    async fn test_mock_embeddings_different_texts() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new());

        let texts = vec!["A".to_string(), "B".to_string()];
        let vectors = embed(embeddings, &texts).await.unwrap();

        // Different texts should generate different vectors
        assert_ne!(vectors[0], vectors[1]);
    }

    #[tokio::test]
    async fn test_mock_embeddings_query() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new());

        let text = "Query text";
        let vector = embed_query(embeddings, text).await.unwrap();

        assert_eq!(vector.len(), 3);

        // Should be normalized
        let magnitude: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_mock_embeddings_custom_dimensions() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::with_dimensions(128));

        let text = vec!["Test".to_string()];
        let vectors = embed(embeddings, &text).await.unwrap();

        assert_eq!(vectors[0].len(), 128);

        // Should be normalized
        let magnitude: f32 = vectors[0].iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_mock_embeddings_empty_text() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::new());

        let text = vec!["".to_string()];
        let vectors = embed(embeddings, &text).await.unwrap();

        assert_eq!(vectors[0].len(), 3);

        // Even empty text should give normalized vector
        let magnitude: f32 = vectors[0].iter().map(|v| v * v).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_mock_embeddings_single_dimension() {
        let embeddings: Arc<dyn Embeddings> = Arc::new(MockEmbeddings::with_dimensions(1));

        let text = vec!["Test".to_string()];
        let vectors = embed(embeddings, &text).await.unwrap();

        assert_eq!(vectors[0].len(), 1);
        assert!((vectors[0][0] - 1.0).abs() < 0.001); // Single dimension normalized to 1.0
    }
}
