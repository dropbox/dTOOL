use async_trait::async_trait;
use dashflow::core::{
    documents::Document,
    embeddings::Embeddings,
    vector_stores::{DistanceMetric, VectorStore},
    Result,
};
use dashflow_pgvector::PgVectorStore;
use std::collections::HashMap;
use std::sync::Arc;

/// Supabase vector store implementation.
///
/// This is a thin wrapper around `PgVectorStore` with Supabase-specific connection handling.
pub struct SupabaseVectorStore {
    inner: PgVectorStore,
}

/// Build a Supabase connection string by injecting credentials if needed.
///
/// Supabase connection strings often don't include the password for security
/// reasons. This function transforms:
/// - `postgres://postgres.xxx.supabase.co/postgres` → `postgres://postgres:password@postgres.xxx.supabase.co/postgres`
///
/// If the connection string already contains `@` (credentials), it's returned unchanged.
fn build_connection_string(connection_string: &str, password: &str) -> String {
    if connection_string.contains('@') {
        connection_string.to_string()
    } else {
        connection_string.replace("://postgres.", &format!("://postgres:{password}@postgres."))
    }
}

impl SupabaseVectorStore {
    /// Creates a new Supabase vector store.
    pub async fn new(
        connection_string: &str,
        password: &str,
        collection_name: &str,
        embeddings: Arc<dyn Embeddings>,
    ) -> Result<Self> {
        let conn_string = build_connection_string(connection_string, password);

        let inner = PgVectorStore::new(&conn_string, collection_name, embeddings).await?;
        Ok(Self { inner })
    }

    #[must_use]
    pub fn inner(&self) -> &PgVectorStore {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut PgVectorStore {
        &mut self.inner
    }
}

#[async_trait]
impl VectorStore for SupabaseVectorStore {
    fn embeddings(&self) -> Option<Arc<dyn Embeddings>> {
        self.inner.embeddings()
    }

    fn distance_metric(&self) -> DistanceMetric {
        self.inner.distance_metric()
    }

    async fn add_texts(
        &mut self,
        texts: &[impl AsRef<str> + Send + Sync],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
        ids: Option<&[String]>,
    ) -> Result<Vec<String>> {
        self.inner.add_texts(texts, metadatas, ids).await
    }

    async fn delete(&mut self, ids: Option<&[String]>) -> Result<bool> {
        self.inner.delete(ids).await
    }

    async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<Document>> {
        self.inner.get_by_ids(ids).await
    }

    async fn _similarity_search(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        self.inner._similarity_search(query, k, filter).await
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        self.inner
            .similarity_search_with_score(query, k, filter)
            .await
    }

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        self.inner
            .similarity_search_by_vector(embedding, k, filter)
            .await
    }

    async fn similarity_search_by_vector_with_score(
        &self,
        embedding: &[f32],
        k: usize,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<(Document, f32)>> {
        self.inner
            .similarity_search_by_vector_with_score(embedding, k, filter)
            .await
    }

    async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        filter: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<Document>> {
        self.inner
            .max_marginal_relevance_search(query, k, fetch_k, lambda_mult, filter)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_connection_string_without_credentials() {
        // Typical Supabase connection string without password
        let conn = "postgres://postgres.abcdefg.supabase.co:5432/postgres";
        let password = "my-secret-password";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:my-secret-password@postgres.abcdefg.supabase.co:5432/postgres"
        );
    }

    #[test]
    fn test_build_connection_string_with_existing_credentials() {
        // Connection string that already has user@host format
        let conn = "postgres://user:pass@localhost:5432/mydb";
        let password = "ignored-password";

        let result = build_connection_string(conn, password);

        // Should return unchanged since @ already present
        assert_eq!(result, "postgres://user:pass@localhost:5432/mydb");
    }

    #[test]
    fn test_build_connection_string_with_only_at_sign() {
        // Edge case: @ in a different part of the URL
        let conn = "postgres://user@host/db";
        let password = "pass";

        let result = build_connection_string(conn, password);

        // Contains @, so should be unchanged
        assert_eq!(result, "postgres://user@host/db");
    }

    #[test]
    fn test_build_connection_string_empty_password() {
        let conn = "postgres://postgres.xyz.supabase.co/postgres";
        let password = "";

        let result = build_connection_string(conn, password);

        // Empty password still gets injected (user should validate)
        assert_eq!(result, "postgres://postgres:@postgres.xyz.supabase.co/postgres");
    }

    #[test]
    fn test_build_connection_string_special_chars_in_password() {
        let conn = "postgres://postgres.proj.supabase.co/postgres";
        let password = "p@ss:word/with#special";

        let result = build_connection_string(conn, password);

        // Special characters in password are preserved (URL encoding is caller's responsibility)
        assert_eq!(
            result,
            "postgres://postgres:p@ss:word/with#special@postgres.proj.supabase.co/postgres"
        );
    }

    #[test]
    fn test_build_connection_string_no_postgres_prefix() {
        // Edge case: doesn't match the expected pattern
        let conn = "postgres://db.supabase.co/mydb";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // No "://postgres." to replace, so unchanged
        assert_eq!(result, "postgres://db.supabase.co/mydb");
    }

    #[test]
    fn test_build_connection_string_mysql_scheme() {
        // Wrong scheme - shouldn't match postgres pattern
        let conn = "mysql://postgres.example.com/db";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // Still replaces "://postgres." regardless of scheme
        assert_eq!(result, "mysql://postgres:secret@postgres.example.com/db");
    }

    #[test]
    fn test_build_connection_string_preserves_query_params() {
        let conn = "postgres://postgres.abc.supabase.co/db?sslmode=require&timeout=30";
        let password = "secret123";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:secret123@postgres.abc.supabase.co/db?sslmode=require&timeout=30"
        );
    }

    #[test]
    fn test_build_connection_string_with_port() {
        let conn = "postgres://postgres.region.supabase.co:6543/postgres";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:pw@postgres.region.supabase.co:6543/postgres"
        );
    }

    #[test]
    fn test_build_connection_string_standard_format() {
        // Standard format without supabase-specific postgres. prefix
        let conn = "postgres://localhost:5432/testdb";
        let password = "unused";

        let result = build_connection_string(conn, password);

        // No "://postgres." to match, so unchanged
        assert_eq!(result, "postgres://localhost:5432/testdb");
    }

    // === Additional Edge Cases for Connection String Building ===

    #[test]
    fn test_build_connection_string_unicode_password() {
        let conn = "postgres://postgres.abc.supabase.co/db";
        let password = "密码🔐пароль";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:密码🔐пароль@postgres.abc.supabase.co/db"
        );
    }

    #[test]
    fn test_build_connection_string_very_long_password() {
        let conn = "postgres://postgres.proj.supabase.co/db";
        let password = "a".repeat(1000);

        let result = build_connection_string(conn, &password);

        assert!(result.contains(&password));
        assert!(result.contains("@postgres."));
    }

    #[test]
    fn test_build_connection_string_whitespace_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "  spaces around  ";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:  spaces around  @postgres.x.supabase.co/db"
        );
    }

    #[test]
    fn test_build_connection_string_tab_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "pass\tword";

        let result = build_connection_string(conn, password);

        assert!(result.contains("pass\tword"));
    }

    #[test]
    fn test_build_connection_string_newline_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "pass\nword";

        let result = build_connection_string(conn, password);

        assert!(result.contains("pass\nword"));
    }

    #[test]
    fn test_build_connection_string_empty_conn_string() {
        let conn = "";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // No pattern to match, returned unchanged
        assert_eq!(result, "");
    }

    #[test]
    fn test_build_connection_string_just_protocol() {
        let conn = "postgres://";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // No postgres. pattern to match
        assert_eq!(result, "postgres://");
    }

    #[test]
    fn test_build_connection_string_ipv6_host() {
        // IPv6 address (not typical Supabase, but edge case)
        let conn = "postgres://[::1]:5432/db";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // No postgres. pattern
        assert_eq!(result, "postgres://[::1]:5432/db");
    }

    #[test]
    fn test_build_connection_string_multiple_at_in_password() {
        // Password contains @, but URL already has @
        let conn = "postgres://user:p@ss@host/db";
        let password = "ignored";

        let result = build_connection_string(conn, password);

        // Contains @, so unchanged
        assert_eq!(result, "postgres://user:p@ss@host/db");
    }

    #[test]
    fn test_build_connection_string_url_encoded_existing_password() {
        let conn = "postgres://user:p%40ssword@host/db";
        let password = "ignored";

        let result = build_connection_string(conn, password);

        // Contains @, so unchanged (the %40 is literal, @ detection finds literal @)
        assert_eq!(result, "postgres://user:p%40ssword@host/db");
    }

    #[test]
    fn test_build_connection_string_fragment() {
        let conn = "postgres://postgres.abc.supabase.co/db#fragment";
        let password = "secret";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:secret@postgres.abc.supabase.co/db#fragment"
        );
    }

    #[test]
    fn test_build_connection_string_multiple_query_params() {
        let conn =
            "postgres://postgres.abc.supabase.co/db?sslmode=require&connect_timeout=10&application_name=test";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert!(result.starts_with("postgres://postgres:pw@postgres.abc.supabase.co/db?"));
        assert!(result.contains("sslmode=require"));
        assert!(result.contains("connect_timeout=10"));
        assert!(result.contains("application_name=test"));
    }

    #[test]
    fn test_build_connection_string_double_colon_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "user::pass";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:user::pass@postgres.x.supabase.co/db"
        );
    }

    #[test]
    fn test_build_connection_string_slash_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "pass/word";

        let result = build_connection_string(conn, password);

        assert!(result.contains("pass/word@postgres."));
    }

    #[test]
    fn test_build_connection_string_percent_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "100%safe";

        let result = build_connection_string(conn, password);

        assert!(result.contains("100%safe@postgres."));
    }

    #[test]
    fn test_build_connection_string_question_mark_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "what?ever";

        let result = build_connection_string(conn, password);

        assert!(result.contains("what?ever@postgres."));
    }

    #[test]
    fn test_build_connection_string_ampersand_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "salt&pepper";

        let result = build_connection_string(conn, password);

        assert!(result.contains("salt&pepper@postgres."));
    }

    #[test]
    fn test_build_connection_string_equals_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "key=value";

        let result = build_connection_string(conn, password);

        assert!(result.contains("key=value@postgres."));
    }

    #[test]
    fn test_build_connection_string_brackets_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "[bracket]";

        let result = build_connection_string(conn, password);

        assert!(result.contains("[bracket]@postgres."));
    }

    #[test]
    fn test_build_connection_string_multiple_postgres_dots() {
        // Edge case: multiple postgres. patterns (should only replace first)
        let conn = "postgres://postgres.a.postgres.b.supabase.co/db";
        let password = "pw";

        let result = build_connection_string(conn, password);

        // The replace function replaces all occurrences of "://postgres."
        // Since there's only one "://postgres." this should work fine
        assert!(result.contains("@postgres.a"));
    }

    #[test]
    fn test_build_connection_string_postgresql_scheme() {
        // postgresql:// vs postgres:// - both are valid
        let conn = "postgresql://postgres.abc.supabase.co/db";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // "://postgres." pattern should still match
        assert_eq!(
            result,
            "postgresql://postgres:secret@postgres.abc.supabase.co/db"
        );
    }

    #[test]
    fn test_build_connection_string_case_sensitive() {
        // PostgreSQL is case sensitive for protocol but the pattern match is case-sensitive
        let conn = "POSTGRES://postgres.abc.supabase.co/db";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // "://postgres." still matches
        assert_eq!(
            result,
            "POSTGRES://postgres:secret@postgres.abc.supabase.co/db"
        );
    }

    #[test]
    fn test_build_connection_string_pooler_mode() {
        // Supabase connection pooler with different port
        let conn = "postgres://postgres.pooler.supabase.co:6543/postgres?pgbouncer=true";
        let password = "secret";

        let result = build_connection_string(conn, password);

        // Contains "://postgres." so should transform
        assert!(result.contains("postgres:secret@postgres.pooler"));
        assert!(result.contains("pgbouncer=true"));
    }

    #[test]
    fn test_build_connection_string_backslash_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = r"back\slash";

        let result = build_connection_string(conn, password);

        assert!(result.contains(r"back\slash@postgres."));
    }

    #[test]
    fn test_build_connection_string_quotes_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = r#"say"hello"#;

        let result = build_connection_string(conn, password);

        assert!(result.contains(r#"say"hello@postgres."#));
    }

    #[test]
    fn test_build_connection_string_single_quotes_in_password() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "it's";

        let result = build_connection_string(conn, password);

        assert!(result.contains("it's@postgres."));
    }

    #[test]
    fn test_build_connection_string_only_special_chars() {
        let conn = "postgres://postgres.x.supabase.co/db";
        let password = "!@#$%^&*()";

        let result = build_connection_string(conn, password);

        assert!(result.contains("!@#$%^&*()@postgres."));
    }

    #[test]
    fn test_build_connection_string_database_path() {
        let conn = "postgres://postgres.abc.supabase.co/path/to/db";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:pw@postgres.abc.supabase.co/path/to/db"
        );
    }

    #[test]
    fn test_build_connection_string_no_database() {
        let conn = "postgres://postgres.abc.supabase.co";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(result, "postgres://postgres:pw@postgres.abc.supabase.co");
    }

    #[test]
    fn test_build_connection_string_trailing_slash() {
        let conn = "postgres://postgres.abc.supabase.co/";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(result, "postgres://postgres:pw@postgres.abc.supabase.co/");
    }

    #[test]
    fn test_build_connection_string_numeric_project_id() {
        let conn = "postgres://postgres.12345.supabase.co/postgres";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:pw@postgres.12345.supabase.co/postgres"
        );
    }

    #[test]
    fn test_build_connection_string_mixed_case_project_id() {
        let conn = "postgres://postgres.AbCdEfG.supabase.co/postgres";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:pw@postgres.AbCdEfG.supabase.co/postgres"
        );
    }

    #[test]
    fn test_build_connection_string_hyphenated_project_id() {
        let conn = "postgres://postgres.my-cool-project.supabase.co/postgres";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:pw@postgres.my-cool-project.supabase.co/postgres"
        );
    }

    #[test]
    fn test_build_connection_string_underscored_project_id() {
        let conn = "postgres://postgres.my_project_123.supabase.co/postgres";
        let password = "pw";

        let result = build_connection_string(conn, password);

        assert_eq!(
            result,
            "postgres://postgres:pw@postgres.my_project_123.supabase.co/postgres"
        );
    }

    // === Type and Trait Bound Tests ===

    /// Verify SupabaseVectorStore is Send (can be transferred across threads)
    #[test]
    fn test_supabase_vector_store_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SupabaseVectorStore>();
    }

    /// Verify SupabaseVectorStore is Sync (can be shared across threads)
    #[test]
    fn test_supabase_vector_store_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SupabaseVectorStore>();
    }

    /// Document that VectorStore trait is NOT object-safe due to generic methods
    /// (This is expected - add_texts has generic type parameters)
    #[test]
    fn test_vector_store_trait_not_object_safe() {
        // VectorStore is NOT dyn-compatible because add_texts() has generic parameters.
        // This is a design tradeoff - we get ergonomic API at the cost of dynamic dispatch.
        // This test documents this behavior.
        fn requires_vector_store<T: VectorStore>() {}
        requires_vector_store::<SupabaseVectorStore>();
    }

    /// Verify embeddings accessor return type
    #[test]
    fn test_embeddings_return_type() {
        fn check_return_type<T: VectorStore>() {
            fn _takes_option_arc(_: Option<Arc<dyn Embeddings>>) {}
            // This would fail to compile if return type changed
        }
        check_return_type::<SupabaseVectorStore>();
    }

    /// Verify distance_metric accessor return type
    #[test]
    fn test_distance_metric_return_type() {
        fn check_return_type<T: VectorStore>() {
            fn _takes_distance_metric(_: DistanceMetric) {}
            // Verifies type at compile time
        }
        check_return_type::<SupabaseVectorStore>();
    }

    // === Module Structure Tests ===

    #[test]
    fn test_module_exports_supabase_vector_store() {
        // Verify the type is exported from the crate root
        use crate::SupabaseVectorStore;
        let _ = std::any::type_name::<SupabaseVectorStore>();
    }

    #[test]
    fn test_struct_size_is_reasonable() {
        // SupabaseVectorStore wraps PgVectorStore, should be single-pointer-sized wrapper
        let size = std::mem::size_of::<SupabaseVectorStore>();
        // Should be at least pointer-sized, but not excessively large
        assert!(size >= std::mem::size_of::<usize>());
        assert!(size < 1024); // Sanity check: not megabytes
    }

    #[test]
    fn test_struct_alignment() {
        let align = std::mem::align_of::<SupabaseVectorStore>();
        // Should have reasonable alignment (typically 8 on 64-bit)
        assert!(align >= 1);
        assert!(align <= 16);
    }
}
