//! Archive format loaders (ZIP, TAR, GZIP)
//!
//! This module contains loaders for compressed and archived file formats.

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Document, DocumentLoader};
use crate::core::error::Result;

/// Loads ZIP archives as documents (lists contents or extracts files).
///
/// The `ZipFileLoader` can either:
/// - List files in the archive (default)
/// - Extract and load all text files as separate documents
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::ZipFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // List files only
/// let loader = ZipFileLoader::new("archive.zip");
/// let documents = loader.load().await?;
///
/// // Extract and load files
/// let loader = ZipFileLoader::new("archive.zip").with_extract(true);
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct ZipFileLoader {
    /// Path to the ZIP file
    pub file_path: PathBuf,
    /// Whether to extract contents (true) or just list files (false)
    pub extract_contents: bool,
}

impl ZipFileLoader {
    /// Create a new `ZipFileLoader` for the given file path.
    ///
    /// By default, only lists file names (does not extract contents).
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            extract_contents: false,
        }
    }

    /// Configure whether to extract file contents or just list files.
    #[must_use]
    pub fn with_extract(mut self, extract: bool) -> Self {
        self.extract_contents = extract;
        self
    }
}

#[async_trait]
impl DocumentLoader for ZipFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let extract_contents = self.extract_contents;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            use zip::ZipArchive;

            let file = std::fs::File::open(&file_path).map_err(crate::core::error::Error::Io)?;
            let mut archive = ZipArchive::new(file)
                .map_err(|e| crate::core::error::Error::InvalidInput(format!("ZIP error: {e}")))?;

            if extract_contents {
                // Extract all files as separate documents
                let mut documents = Vec::new();

                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).map_err(|e| {
                        crate::core::error::Error::InvalidInput(format!("ZIP error: {e}"))
                    })?;

                    if !file.is_dir() {
                        let mut contents = String::new();
                        if file.read_to_string(&mut contents).is_ok() {
                            let doc = Document::new(contents)
                                .with_metadata(
                                    "source",
                                    format!("{}:{}", file_path.display(), file.name()),
                                )
                                .with_metadata("format", "zip_extracted")
                                .with_metadata("original_name", file.name());
                            documents.push(doc);
                        }
                    }
                }

                Ok(documents)
            } else {
                // Just list files
                let mut file_list = Vec::new();
                for i in 0..archive.len() {
                    let file = archive.by_index(i).map_err(|e| {
                        crate::core::error::Error::InvalidInput(format!("ZIP error: {e}"))
                    })?;
                    file_list.push(format!("{} ({} bytes)", file.name(), file.size()));
                }

                let content = format!(
                    "ZIP archive: {}\nFiles:\n{}",
                    file_path.display(),
                    file_list.join("\n")
                );

                let doc = Document::new(content)
                    .with_metadata("source", file_path.display().to_string())
                    .with_metadata("format", "zip")
                    .with_metadata("file_count", archive.len().to_string());

                Ok(vec![doc])
            }
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads TAR archives as documents (lists contents or extracts files).
///
/// The `TarFileLoader` can either:
/// - List files in the archive (default)
/// - Extract and load all text files as separate documents
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::TarFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TarFileLoader::new("archive.tar");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TarFileLoader {
    /// Path to the TAR file
    pub file_path: PathBuf,
    /// Whether to extract contents (true) or just list files (false)
    pub extract_contents: bool,
}

impl TarFileLoader {
    /// Create a new `TarFileLoader` for the given file path.
    ///
    /// By default, only lists file names (does not extract contents).
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            extract_contents: false,
        }
    }

    /// Configure whether to extract file contents or just list files.
    #[must_use]
    pub fn with_extract(mut self, extract: bool) -> Self {
        self.extract_contents = extract;
        self
    }
}

#[async_trait]
impl DocumentLoader for TarFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();
        let extract_contents = self.extract_contents;

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            use tar::Archive;

            let file = std::fs::File::open(&file_path).map_err(crate::core::error::Error::Io)?;
            let mut archive = Archive::new(file);

            if extract_contents {
                // Extract all text files - need to re-open for entries iteration
                let file =
                    std::fs::File::open(&file_path).map_err(crate::core::error::Error::Io)?;
                let mut archive = Archive::new(file);
                let mut documents = Vec::new();

                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let path = entry.path()?.to_path_buf();

                    if !entry.header().entry_type().is_dir() {
                        let mut contents = String::new();
                        if entry.read_to_string(&mut contents).is_ok() {
                            let doc = Document::new(contents)
                                .with_metadata(
                                    "source",
                                    format!("{}:{}", file_path.display(), path.display()),
                                )
                                .with_metadata("format", "tar_extracted")
                                .with_metadata("original_name", path.display().to_string());
                            documents.push(doc);
                        }
                    }
                }

                Ok(documents)
            } else {
                // Just list files
                let mut file_list = Vec::new();
                for entry in archive.entries()? {
                    let entry = entry?;
                    if let Ok(path) = entry.path() {
                        file_list.push(format!("{} ({} bytes)", path.display(), entry.size()));
                    }
                }

                let content = format!(
                    "TAR archive: {}\nFiles:\n{}",
                    file_path.display(),
                    file_list.join("\n")
                );

                let doc = Document::new(content)
                    .with_metadata("source", file_path.display().to_string())
                    .with_metadata("format", "tar")
                    .with_metadata("file_count", file_list.len().to_string());

                Ok(vec![doc])
            }
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

/// Loads GZIP compressed files by decompressing and loading the content.
///
/// The `GzipFileLoader` decompresses GZIP files and loads the content as a single document.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::GzipFileLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = GzipFileLoader::new("file.gz");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GzipFileLoader {
    /// Path to the GZIP file
    pub file_path: PathBuf,
}

impl GzipFileLoader {
    /// Create a new `GzipFileLoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
        }
    }
}

/// Maximum allowed decompressed document size: 100 MB (protection against gzip bombs)
const MAX_DECOMPRESSED_DOCUMENT_SIZE: u64 = 100 * 1024 * 1024;

#[async_trait]
impl DocumentLoader for GzipFileLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let file_path = self.file_path.clone();

        // Use spawn_blocking to avoid blocking the async runtime with std::fs I/O
        tokio::task::spawn_blocking(move || {
            use flate2::read::GzDecoder;
            use std::io::Read;

            let file = std::fs::File::open(&file_path).map_err(crate::core::error::Error::Io)?;
            let decoder = GzDecoder::new(file);
            // Limit decompressed size to prevent gzip bombs
            let mut limited = decoder.take(MAX_DECOMPRESSED_DOCUMENT_SIZE + 1);
            let mut decompressed = String::new();

            limited.read_to_string(&mut decompressed).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("GZIP decompression error: {e}"))
            })?;

            if decompressed.len() as u64 > MAX_DECOMPRESSED_DOCUMENT_SIZE {
                return Err(crate::core::error::Error::InvalidInput(format!(
                    "Decompressed document size exceeds limit of {} bytes",
                    MAX_DECOMPRESSED_DOCUMENT_SIZE
                )));
            }

            let doc = Document::new(decompressed)
                .with_metadata("source", file_path.display().to_string())
                .with_metadata("format", "gzip");

            Ok(vec![doc])
        })
        .await
        .map_err(|e| crate::core::error::Error::Other(format!("Task join error: {e}")))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    // ==========================================================================
    // ZipFileLoader Tests
    // ==========================================================================

    #[test]
    fn test_zip_file_loader_new() {
        let loader = ZipFileLoader::new("test.zip");
        assert_eq!(loader.file_path, PathBuf::from("test.zip"));
        assert!(!loader.extract_contents);
    }

    #[test]
    fn test_zip_file_loader_with_extract() {
        let loader = ZipFileLoader::new("test.zip").with_extract(true);
        assert!(loader.extract_contents);

        let loader2 = ZipFileLoader::new("test.zip").with_extract(false);
        assert!(!loader2.extract_contents);
    }

    #[test]
    fn test_zip_file_loader_clone() {
        let loader = ZipFileLoader::new("test.zip").with_extract(true);
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
        assert_eq!(cloned.extract_contents, loader.extract_contents);
    }

    #[test]
    fn test_zip_file_loader_debug() {
        let loader = ZipFileLoader::new("test.zip");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("ZipFileLoader"));
        assert!(debug_str.contains("test.zip"));
    }

    #[tokio::test]
    async fn test_zip_file_loader_list_mode() {
        // Create a simple ZIP file
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();

            zip.start_file("file1.txt", options).unwrap();
            zip.write_all(b"Hello, World!").unwrap();

            zip.start_file("file2.txt", options).unwrap();
            zip.write_all(b"Second file").unwrap();

            zip.finish().unwrap();
        }

        let loader = ZipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("file1.txt"));
        assert!(docs[0].page_content.contains("file2.txt"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "zip");
        assert_eq!(docs[0].metadata.get("file_count").unwrap(), "2");
    }

    #[tokio::test]
    async fn test_zip_file_loader_extract_mode() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut zip = zip::ZipWriter::new(file);
            let options = zip::write::SimpleFileOptions::default();

            zip.start_file("hello.txt", options).unwrap();
            zip.write_all(b"Hello from ZIP!").unwrap();

            zip.start_file("goodbye.txt", options).unwrap();
            zip.write_all(b"Goodbye from ZIP!").unwrap();

            zip.finish().unwrap();
        }

        let loader = ZipFileLoader::new(&path).with_extract(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].metadata.get("format").unwrap(), "zip_extracted");

        // Check that content was extracted
        let contents: Vec<&str> = docs.iter().map(|d| d.page_content.as_str()).collect();
        assert!(contents.contains(&"Hello from ZIP!"));
        assert!(contents.contains(&"Goodbye from ZIP!"));
    }

    #[tokio::test]
    async fn test_zip_file_loader_empty_archive() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let zip = zip::ZipWriter::new(file);
            zip.finish().unwrap();
        }

        let loader = ZipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("file_count").unwrap(), "0");
    }

    #[tokio::test]
    async fn test_zip_file_loader_file_not_found() {
        let loader = ZipFileLoader::new("/nonexistent/path/file.zip");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_zip_file_loader_invalid_zip() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"not a zip file").unwrap();

        let loader = ZipFileLoader::new(temp_file.path());
        let result = loader.load().await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // TarFileLoader Tests
    // ==========================================================================

    #[test]
    fn test_tar_file_loader_new() {
        let loader = TarFileLoader::new("test.tar");
        assert_eq!(loader.file_path, PathBuf::from("test.tar"));
        assert!(!loader.extract_contents);
    }

    #[test]
    fn test_tar_file_loader_with_extract() {
        let loader = TarFileLoader::new("test.tar").with_extract(true);
        assert!(loader.extract_contents);
    }

    #[test]
    fn test_tar_file_loader_clone() {
        let loader = TarFileLoader::new("test.tar").with_extract(true);
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
        assert_eq!(cloned.extract_contents, loader.extract_contents);
    }

    #[test]
    fn test_tar_file_loader_debug() {
        let loader = TarFileLoader::new("test.tar");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("TarFileLoader"));
    }

    #[tokio::test]
    async fn test_tar_file_loader_list_mode() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut builder = tar::Builder::new(file);

            // Add a file to the tar
            let temp_dir = TempDir::new().unwrap();
            let file1_path = temp_dir.path().join("test1.txt");
            std::fs::write(&file1_path, "Test content 1").unwrap();

            builder.append_path_with_name(&file1_path, "test1.txt").unwrap();
            builder.finish().unwrap();
        }

        let loader = TarFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("test1.txt"));
        assert_eq!(docs[0].metadata.get("format").unwrap(), "tar");
    }

    #[tokio::test]
    async fn test_tar_file_loader_extract_mode() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut builder = tar::Builder::new(file);

            let temp_dir = TempDir::new().unwrap();
            let file1_path = temp_dir.path().join("hello.txt");
            std::fs::write(&file1_path, "Hello from TAR!").unwrap();

            builder.append_path_with_name(&file1_path, "hello.txt").unwrap();
            builder.finish().unwrap();
        }

        let loader = TarFileLoader::new(&path).with_extract(true);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello from TAR!");
        assert_eq!(docs[0].metadata.get("format").unwrap(), "tar_extracted");
    }

    #[tokio::test]
    async fn test_tar_file_loader_empty_archive() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut builder = tar::Builder::new(file);
            builder.finish().unwrap();
        }

        let loader = TarFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].metadata.get("file_count").unwrap(), "0");
    }

    #[tokio::test]
    async fn test_tar_file_loader_file_not_found() {
        let loader = TarFileLoader::new("/nonexistent/path/file.tar");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    // ==========================================================================
    // GzipFileLoader Tests
    // ==========================================================================

    #[test]
    fn test_gzip_file_loader_new() {
        let loader = GzipFileLoader::new("test.gz");
        assert_eq!(loader.file_path, PathBuf::from("test.gz"));
    }

    #[test]
    fn test_gzip_file_loader_clone() {
        let loader = GzipFileLoader::new("test.gz");
        let cloned = loader.clone();
        assert_eq!(cloned.file_path, loader.file_path);
    }

    #[test]
    fn test_gzip_file_loader_debug() {
        let loader = GzipFileLoader::new("test.gz");
        let debug_str = format!("{:?}", loader);
        assert!(debug_str.contains("GzipFileLoader"));
    }

    #[tokio::test]
    async fn test_gzip_file_loader_decompress() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(b"Hello, compressed world!").unwrap();
            encoder.finish().unwrap();
        }

        let loader = GzipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello, compressed world!");
        assert_eq!(docs[0].metadata.get("format").unwrap(), "gzip");
    }

    #[tokio::test]
    async fn test_gzip_file_loader_empty_file() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(b"").unwrap();
            encoder.finish().unwrap();
        }

        let loader = GzipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.is_empty());
    }

    #[tokio::test]
    async fn test_gzip_file_loader_file_not_found() {
        let loader = GzipFileLoader::new("/nonexistent/path/file.gz");
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_gzip_file_loader_invalid_gzip() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"not a gzip file").unwrap();

        let loader = GzipFileLoader::new(temp_file.path());
        let result = loader.load().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_gzip_file_loader_multiline_content() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        let content = "Line 1\nLine 2\nLine 3\n";
        {
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(content.as_bytes()).unwrap();
            encoder.finish().unwrap();
        }

        let loader = GzipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, content);
    }

    #[test]
    fn test_max_decompressed_size_constant() {
        // Verify the constant is 100 MB
        assert_eq!(MAX_DECOMPRESSED_DOCUMENT_SIZE, 100 * 1024 * 1024);
    }

    // ==========================================================================
    // Source Metadata Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_zip_loader_source_metadata() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let zip = zip::ZipWriter::new(file);
            zip.finish().unwrap();
        }

        let loader = ZipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        let source = docs[0].metadata.get("source").unwrap().to_string();
        assert!(source.contains(path.file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_tar_loader_source_metadata() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut builder = tar::Builder::new(file);
            builder.finish().unwrap();
        }

        let loader = TarFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        let source = docs[0].metadata.get("source").unwrap().to_string();
        assert!(source.contains(path.file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn test_gzip_loader_source_metadata() {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let file = std::fs::File::create(&path).unwrap();
            let mut encoder = GzEncoder::new(file, Compression::default());
            encoder.write_all(b"test").unwrap();
            encoder.finish().unwrap();
        }

        let loader = GzipFileLoader::new(&path);
        let docs = loader.load().await.unwrap();

        let source = docs[0].metadata.get("source").unwrap().to_string();
        assert!(source.contains(path.file_name().unwrap().to_str().unwrap()));
    }

    // ==========================================================================
    // DocumentLoader Trait Tests
    // ==========================================================================

    #[test]
    fn test_loaders_implement_document_loader() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<ZipFileLoader>();
        _assert_document_loader::<TarFileLoader>();
        _assert_document_loader::<GzipFileLoader>();
    }
}
