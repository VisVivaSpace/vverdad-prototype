//! File source abstraction for reading from directories or zip archives
//!
//! Provides a unified interface for reading project data from either:
//! - Local filesystem directories
//! - `.vv` zip archives (which are just zip files with a different extension)
//!
//! Also provides OutputSink for writing output to either filesystem or zip.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::FileOptions;

use crate::error::VVError;

// =============================================================================
// FileSource Enum (DOP Principle 2: Generic Data Structures)
// =============================================================================

/// Represents an entry in a directory listing
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
}

/// Unified file source — enum dispatches to directory or zip implementations.
/// Replaces the former `FileSource` trait + `Box<dyn FileSource>` pattern.
pub enum FileSource {
    Directory(DirectorySource),
    Zip(ZipSource),
}

/// Lists entries in a directory within the source
pub fn read_dir(source: &FileSource, path: &Path) -> Result<Vec<DirEntry>, VVError> {
    match source {
        FileSource::Directory(s) => read_dir_filesystem(s, path),
        FileSource::Zip(s) => read_dir_zip(s, path),
    }
}

/// Reads file contents as bytes from the source
pub fn read_file(source: &FileSource, path: &Path) -> Result<Vec<u8>, VVError> {
    match source {
        FileSource::Directory(s) => read_file_filesystem(s, path),
        FileSource::Zip(s) => read_file_zip(s, path),
    }
}

/// Checks if path is a directory within the source
pub fn is_dir(source: &FileSource, path: &Path) -> bool {
    match source {
        FileSource::Directory(_) => path.is_dir(),
        FileSource::Zip(s) => is_dir_zip(s, path),
    }
}

/// Checks if path is a file within the source
pub fn is_file(source: &FileSource, path: &Path) -> bool {
    match source {
        FileSource::Directory(_) => path.is_file(),
        FileSource::Zip(s) => is_file_zip(s, path),
    }
}

/// Returns the root path of this source
pub fn source_root(source: &FileSource) -> &Path {
    match source {
        FileSource::Directory(s) => &s.root,
        FileSource::Zip(s) => &s.archive_path,
    }
}

// =============================================================================
// DirectorySource - Filesystem Implementation
// =============================================================================

/// File source backed by the local filesystem
pub struct DirectorySource {
    pub root: PathBuf,
}

impl DirectorySource {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

fn read_dir_filesystem(_source: &DirectorySource, path: &Path) -> Result<Vec<DirEntry>, VVError> {
    let entries = fs::read_dir(path)?;
    let mut result = Vec::new();

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_str().unwrap_or("?").to_string();
        let is_dir = path.is_dir();

        result.push(DirEntry { path, name, is_dir });
    }

    Ok(result)
}

fn read_file_filesystem(_source: &DirectorySource, path: &Path) -> Result<Vec<u8>, VVError> {
    Ok(fs::read(path)?)
}

// =============================================================================
// ZipSource - Zip Archive Implementation (DOP Principle 3: Immutable Data)
// =============================================================================

/// Metadata about a zip entry
#[derive(Debug, Clone)]
struct ZipEntryInfo {
    /// Whether this is a directory
    is_dir: bool,
}

/// File source backed by a zip archive.
///
/// All file contents are pre-read into memory during construction,
/// eliminating the need for a Mutex around ZipArchive.
pub struct ZipSource {
    pub archive_path: PathBuf,
    /// Map from normalized path to entry metadata
    entries: HashMap<PathBuf, ZipEntryInfo>,
    /// Map from normalized path to file contents (directories excluded)
    file_contents: HashMap<PathBuf, Vec<u8>>,
}

impl ZipSource {
    /// Creates a new ZipSource from a .vv archive path.
    ///
    /// Pre-reads all file contents into memory for pure, Mutex-free access.
    pub fn new(archive_path: PathBuf) -> Result<Self, VVError> {
        let file = File::open(&archive_path)?;
        let mut archive = ZipArchive::new(file)?;

        let mut entries = HashMap::new();
        let mut file_contents = HashMap::new();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i)?;
            let raw_path = entry.name().to_string();
            let normalized = normalize_zip_path(&raw_path);
            let is_dir = entry.is_dir();

            entries.insert(normalized.clone(), ZipEntryInfo { is_dir });

            if !is_dir {
                let mut contents = Vec::new();
                entry.read_to_end(&mut contents)?;
                file_contents.insert(normalized, contents);
            }
        }

        // Add implicit directories (zip files may not have explicit dir entries)
        let paths: Vec<PathBuf> = entries.keys().cloned().collect();
        for path in paths {
            let mut current = path.as_path();
            while let Some(parent) = current.parent() {
                if parent.as_os_str().is_empty() {
                    break;
                }
                if !entries.contains_key(parent) {
                    entries.insert(parent.to_path_buf(), ZipEntryInfo { is_dir: true });
                }
                current = parent;
            }
        }

        Ok(Self {
            archive_path,
            entries,
            file_contents,
        })
    }
}

fn read_dir_zip(source: &ZipSource, path: &Path) -> Result<Vec<DirEntry>, VVError> {
    let normalized = if path == source.archive_path.as_path() {
        PathBuf::new()
    } else {
        path.strip_prefix(&source.archive_path)
            .unwrap_or(path)
            .to_path_buf()
    };

    let mut result = Vec::new();

    for (entry_path, info) in &source.entries {
        if let Some(parent) = entry_path.parent() {
            if parent == normalized.as_path() {
                let name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();

                let full_path = source.archive_path.join(entry_path);

                result.push(DirEntry {
                    path: full_path,
                    name,
                    is_dir: info.is_dir,
                });
            }
        } else if normalized.as_os_str().is_empty() && entry_path.parent().is_none() {
            let name = entry_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string();

            let full_path = source.archive_path.join(entry_path);

            result.push(DirEntry {
                path: full_path,
                name,
                is_dir: info.is_dir,
            });
        }
    }

    // Sort for consistent ordering
    result.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(result)
}

fn read_file_zip(source: &ZipSource, path: &Path) -> Result<Vec<u8>, VVError> {
    let relative = path.strip_prefix(&source.archive_path).unwrap_or(path);

    // Check entry exists and is a file
    let info = source
        .entries
        .get(relative)
        .ok_or_else(|| VVError::FileNotFound(path.to_path_buf()))?;

    if info.is_dir {
        return Err(VVError::NotAFile(path.to_path_buf()));
    }

    source
        .file_contents
        .get(relative)
        .cloned()
        .ok_or_else(|| VVError::FileNotFound(path.to_path_buf()))
}

fn is_dir_zip(source: &ZipSource, path: &Path) -> bool {
    let relative = path.strip_prefix(&source.archive_path).unwrap_or(path);
    source
        .entries
        .get(relative)
        .map(|info| info.is_dir)
        .unwrap_or(false)
}

fn is_file_zip(source: &ZipSource, path: &Path) -> bool {
    let relative = path.strip_prefix(&source.archive_path).unwrap_or(path);
    source
        .entries
        .get(relative)
        .map(|info| !info.is_dir)
        .unwrap_or(false)
}

/// Normalizes a zip path (which uses `/`) to a platform PathBuf
fn normalize_zip_path(zip_path: &str) -> PathBuf {
    // Strip trailing slash for directories
    let path_str = zip_path.trim_end_matches('/');
    // Split on `/` and build PathBuf
    let components: Vec<&str> = path_str.split('/').filter(|s| !s.is_empty()).collect();
    components.iter().collect()
}

/// Converts a PathBuf back to a zip-style path string
fn path_to_zip_path(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_str().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("/")
}

// =============================================================================
// OutputSink Enum (DOP Principle 2: Generic Data Structures)
// =============================================================================

/// Unified output sink — enum dispatches to directory or zip implementations.
/// Replaces the former `OutputSink` trait + `Box<dyn OutputSink>` pattern.
///
/// ZipSink is large (contains ZipWriter buffers) but OutputSink is a singleton
/// ECS Resource created once per run — the size difference doesn't matter.
#[allow(clippy::large_enum_variant)]
pub enum OutputSink {
    Directory(DirectorySink),
    Zip(ZipSink),
}

/// Writes content to a file within the sink
pub fn write_file(sink: &mut OutputSink, path: &Path, content: &[u8]) -> Result<(), VVError> {
    match sink {
        OutputSink::Directory(s) => write_file_filesystem(s, path, content),
        OutputSink::Zip(s) => write_file_zip(s, path, content),
    }
}

/// Creates a directory within the sink
pub fn create_dir(sink: &mut OutputSink, path: &Path) -> Result<(), VVError> {
    match sink {
        OutputSink::Directory(s) => create_dir_filesystem(s, path),
        OutputSink::Zip(s) => create_dir_zip(s, path),
    }
}

/// Returns the root path of this sink
pub fn sink_root(sink: &OutputSink) -> &Path {
    match sink {
        OutputSink::Directory(s) => &s.root,
        OutputSink::Zip(s) => &s.archive_path,
    }
}

/// Flushes any buffered data
pub fn flush_sink(sink: &mut OutputSink) -> Result<(), VVError> {
    match sink {
        OutputSink::Directory(_) => Ok(()),
        OutputSink::Zip(s) => flush_zip(s),
    }
}

// =============================================================================
// DirectorySink - Filesystem Implementation
// =============================================================================

/// Output sink that writes to the local filesystem
pub struct DirectorySink {
    pub root: PathBuf,
}

impl DirectorySink {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

fn write_file_filesystem(
    _sink: &DirectorySink,
    path: &Path,
    content: &[u8],
) -> Result<(), VVError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

fn create_dir_filesystem(_sink: &DirectorySink, path: &Path) -> Result<(), VVError> {
    fs::create_dir_all(path)?;
    Ok(())
}

// =============================================================================
// ZipSink - Zip Archive Implementation (DOP Principle 3: No Hidden Mutable State)
// =============================================================================

/// Output sink that writes to a zip archive.
///
/// Writes all output to _output/ prefix within the archive.
/// No Mutex — requires `&mut self` for write operations.
pub struct ZipSink {
    pub archive_path: PathBuf,
    pub output_prefix: PathBuf,
    writer: Option<ZipWriter<File>>,
}

impl ZipSink {
    /// Creates a new ZipSink that appends to an existing archive
    pub fn new(archive_path: PathBuf, output_prefix: PathBuf) -> Result<Self, VVError> {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&archive_path)?;
        let writer = ZipWriter::new_append(file)?;

        Ok(Self {
            archive_path,
            output_prefix,
            writer: Some(writer),
        })
    }

    /// Creates a new ZipSink that creates a fresh archive (overwrites if exists)
    pub fn create(archive_path: PathBuf, output_prefix: PathBuf) -> Result<Self, VVError> {
        let file = File::create(&archive_path)?;
        let writer = ZipWriter::new(file);

        Ok(Self {
            archive_path,
            output_prefix,
            writer: Some(writer),
        })
    }

    /// Adds a directory entry directly to the archive at the given path (no _output/ prefix)
    pub fn add_raw_directory(&mut self, zip_path: &str) -> Result<(), VVError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| VVError::ArchiveFinished(self.archive_path.clone()))?;
        let dir_path = if zip_path.ends_with('/') {
            zip_path.to_string()
        } else {
            format!("{}/", zip_path)
        };
        let options = FileOptions::<()>::default();
        writer.add_directory(&dir_path, options)?;
        Ok(())
    }

    /// Writes a file directly to the archive at the given path (no _output/ prefix)
    pub fn write_raw(&mut self, zip_path: &str, content: &[u8]) -> Result<(), VVError> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| VVError::ArchiveFinished(self.archive_path.clone()))?;
        let options = FileOptions::<()>::default();
        writer.start_file(zip_path, options)?;
        writer.write_all(content)?;
        Ok(())
    }

    /// Finalizes the archive (must be called before dropping for proper finalization)
    pub fn finish(&mut self) -> Result<(), VVError> {
        if let Some(writer) = self.writer.take() {
            writer.finish()?;
        }
        Ok(())
    }
}

fn write_file_zip(sink: &mut ZipSink, path: &Path, content: &[u8]) -> Result<(), VVError> {
    let relative = path.strip_prefix(&sink.output_prefix).unwrap_or(path);
    let zip_path = format!("_output/{}", path_to_zip_path(relative));

    let writer = sink
        .writer
        .as_mut()
        .ok_or_else(|| VVError::ArchiveFinished(sink.archive_path.clone()))?;
    let options = FileOptions::<()>::default();
    writer.start_file(&zip_path, options)?;
    writer.write_all(content)?;
    Ok(())
}

fn create_dir_zip(sink: &mut ZipSink, path: &Path) -> Result<(), VVError> {
    let relative = path.strip_prefix(&sink.output_prefix).unwrap_or(path);
    let zip_path = format!("_output/{}/", path_to_zip_path(relative));

    let writer = sink
        .writer
        .as_mut()
        .ok_or_else(|| VVError::ArchiveFinished(sink.archive_path.clone()))?;
    let options = FileOptions::<()>::default();
    writer.add_directory(&zip_path, options)?;
    Ok(())
}

fn flush_zip(sink: &mut ZipSink) -> Result<(), VVError> {
    if let Some(writer) = sink.writer.as_mut() {
        writer.flush()?;
    }
    Ok(())
}

impl Drop for ZipSink {
    fn drop(&mut self) {
        if let Some(writer) = self.writer.take() {
            let _ = writer.finish();
        }
    }
}

// =============================================================================
// Project Copying Functions
// =============================================================================

/// Copies all files from a FileSource to a directory on disk
///
/// Excludes _output/ directories from the copy.
pub fn copy_project_to_dir(source: &FileSource, dest: &Path) -> Result<(), VVError> {
    copy_dir_recursive(source, source_root(source), dest, "")
}

/// Recursively copies a directory from FileSource to filesystem
fn copy_dir_recursive(
    source: &FileSource,
    src_path: &Path,
    dest_root: &Path,
    relative_path: &str,
) -> Result<(), VVError> {
    let entries = read_dir(source, src_path)?;

    for entry in entries {
        // Skip _output directories
        if entry.name == "_output" {
            continue;
        }

        let entry_relative = if relative_path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", relative_path, entry.name)
        };

        let dest_path = dest_root.join(&entry_relative);

        if entry.is_dir {
            fs::create_dir_all(&dest_path)?;
            copy_dir_recursive(source, &entry.path, dest_root, &entry_relative)?;
        } else {
            // Ensure parent directory exists
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let content = read_file(source, &entry.path)?;
            fs::write(&dest_path, content)?;
        }
    }

    Ok(())
}

/// Copies all files from a FileSource to a new zip archive
///
/// Excludes _output/ directories from the copy.
/// Returns the ZipSink for writing additional files (like rendered templates).
pub fn copy_project_to_archive(
    source: &FileSource,
    archive_path: &Path,
) -> Result<ZipSink, VVError> {
    let mut sink = ZipSink::create(archive_path.to_path_buf(), PathBuf::from("_output"))?;
    copy_to_archive_recursive(source, source_root(source), &mut sink, "")?;
    Ok(sink)
}

/// Recursively copies files from FileSource to ZipSink
fn copy_to_archive_recursive(
    source: &FileSource,
    src_path: &Path,
    sink: &mut ZipSink,
    relative_path: &str,
) -> Result<(), VVError> {
    let entries = read_dir(source, src_path)?;

    for entry in entries {
        // Skip _output directories
        if entry.name == "_output" {
            continue;
        }

        let entry_relative = if relative_path.is_empty() {
            entry.name.clone()
        } else {
            format!("{}/{}", relative_path, entry.name)
        };

        if entry.is_dir {
            // Add directory entry to zip
            sink.add_raw_directory(&entry_relative)?;

            copy_to_archive_recursive(source, &entry.path, sink, &entry_relative)?;
        } else {
            let content = read_file(source, &entry.path)?;
            sink.write_raw(&entry_relative, &content)?;
        }
    }

    Ok(())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // =========================================================================
    // DirectorySink Tests
    // =========================================================================

    #[test]
    fn test_directory_sink_write_file() {
        let temp = TempDir::new().unwrap();
        let mut sink = OutputSink::Directory(DirectorySink::new(temp.path().to_path_buf()));

        let file_path = temp.path().join("test.txt");
        write_file(&mut sink, &file_path, b"hello world").expect("Failed to write file");

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_directory_sink_write_nested() {
        let temp = TempDir::new().unwrap();
        let mut sink = OutputSink::Directory(DirectorySink::new(temp.path().to_path_buf()));

        let file_path = temp.path().join("a/b/c/test.txt");
        write_file(&mut sink, &file_path, b"nested content").expect("Failed to write nested file");

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "nested content");
    }

    #[test]
    fn test_directory_sink_create_dir() {
        let temp = TempDir::new().unwrap();
        let mut sink = OutputSink::Directory(DirectorySink::new(temp.path().to_path_buf()));

        let dir_path = temp.path().join("new_dir/nested");
        create_dir(&mut sink, &dir_path).expect("Failed to create dir");

        assert!(dir_path.exists());
        assert!(dir_path.is_dir());
    }

    #[test]
    fn test_normalize_zip_path() {
        assert_eq!(
            normalize_zip_path("foo/bar.txt"),
            PathBuf::from("foo/bar.txt")
        );
        assert_eq!(normalize_zip_path("foo/bar/"), PathBuf::from("foo/bar"));
        assert_eq!(
            normalize_zip_path("simple.txt"),
            PathBuf::from("simple.txt")
        );
    }

    #[test]
    fn test_path_to_zip_path() {
        assert_eq!(path_to_zip_path(Path::new("foo/bar.txt")), "foo/bar.txt");
        assert_eq!(path_to_zip_path(Path::new("simple.txt")), "simple.txt");
    }

    // =========================================================================
    // ZipSink Tests - Require temporary zip creation
    // =========================================================================

    #[test]
    fn test_zip_sink_write_file() {
        use std::io::Write as IoWrite;

        let temp = TempDir::new().unwrap();
        let archive_path = temp.path().join("test.vv");

        // Create a minimal zip file first
        {
            let file = File::create(&archive_path).unwrap();
            let mut writer = ZipWriter::new(file);
            writer
                .start_file::<_, ()>("placeholder.txt", FileOptions::default())
                .unwrap();
            writer.write_all(b"placeholder").unwrap();
            writer.finish().unwrap();
        }

        // Now test the sink
        {
            let mut sink = OutputSink::Zip(
                ZipSink::new(archive_path.clone(), temp.path().to_path_buf())
                    .expect("Failed to create sink"),
            );
            let file_path = temp.path().join("output.txt");
            write_file(&mut sink, &file_path, b"test content").expect("Failed to write file");
            flush_sink(&mut sink).expect("Failed to flush");
        }

        // Verify the file was added
        let file = File::open(&archive_path).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        // Look for our file
        let mut found = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            if entry.name().contains("output.txt") {
                found = true;
                break;
            }
        }
        assert!(found, "Expected output.txt in archive");
    }
}
