//! Source-free, integrity-checked session bundle persistence.

use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use sha2::{Digest, Sha256};

const BUNDLE_SCHEMA: &str = "strata-session-bundle";
const BUNDLE_VERSION: u32 = 1;
const MANIFEST_FILE: &str = "manifest.json";
const JOURNAL_FILE: &str = "journal.ndjson";

/// A stable fingerprint for an external source without retaining its location or bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFingerprint {
    alias: String,
    byte_length: u64,
    sha256: String,
}

impl SourceFingerprint {
    /// Builds a checked source fingerprint from a display alias, byte length, and SHA-256 digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the alias is empty or the digest is not lowercase SHA-256 hex.
    pub fn new(
        alias: impl Into<String>,
        byte_length: u64,
        sha256: impl Into<String>,
    ) -> Result<Self, SessionBundleError> {
        let alias = alias.into();
        if alias.trim().is_empty() {
            return Err(SessionBundleError::InvalidFingerprint(
                "source alias must not be empty".to_owned(),
            ));
        }
        if alias.contains(['/', '\\']) {
            return Err(SessionBundleError::InvalidFingerprint(
                "source alias must not contain path separators".to_owned(),
            ));
        }
        let sha256 = sha256.into();
        if !is_sha256_hex(&sha256) {
            return Err(SessionBundleError::InvalidFingerprint(
                "source SHA-256 must be 64 lowercase hexadecimal characters".to_owned(),
            ));
        }
        Ok(Self {
            alias,
            byte_length,
            sha256,
        })
    }

    /// Builds a fingerprint from in-memory source bytes while storing only their digest and length.
    ///
    /// # Errors
    ///
    /// Returns an error when the alias is invalid or the byte length cannot be represented.
    pub fn from_bytes(alias: impl Into<String>, bytes: &[u8]) -> Result<Self, SessionBundleError> {
        let byte_length = u64::try_from(bytes.len())
            .map_err(|_| SessionBundleError::NumericOverflow("source byte length".to_owned()))?;
        Self::new(alias, byte_length, sha256_hex(bytes))
    }

    /// Returns the caller-provided display alias.
    #[must_use]
    pub fn alias(&self) -> &str {
        &self.alias
    }

    /// Returns the source byte length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Returns the lowercase SHA-256 digest.
    #[must_use]
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

impl Serialize for SourceFingerprint {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SourceFingerprintWire {
            alias: &self.alias,
            byte_length: self.byte_length,
            sha256: &self.sha256,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SourceFingerprint {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SourceFingerprintOwnedWire::deserialize(deserializer)?;
        Self::new(wire.alias, wire.byte_length, wire.sha256).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct SourceFingerprintWire<'a> {
    alias: &'a str,
    byte_length: u64,
    sha256: &'a str,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFingerprintOwnedWire {
    alias: String,
    byte_length: u64,
    sha256: String,
}

/// A validated JSON object kept opaque to the session-bundle layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSnapshot(Value);

impl WorkspaceSnapshot {
    /// Parses an opaque JSON value from bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when `bytes` is not valid JSON.
    pub fn from_json_bytes(bytes: &[u8]) -> Result<Self, SessionBundleError> {
        let value = serde_json::from_slice(bytes).map_err(SessionBundleError::Json)?;
        Ok(Self(value))
    }

    /// Parses an opaque JSON value from text.
    ///
    /// # Errors
    ///
    /// Returns an error when `value` is not valid JSON.
    pub fn from_json_str(value: &str) -> Result<Self, SessionBundleError> {
        Self::from_json_bytes(value.as_bytes())
    }

    /// Creates a snapshot from a JSON value.
    #[must_use]
    pub const fn from_value(value: Value) -> Self {
        Self(value)
    }

    /// Returns the opaque JSON value.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.0
    }

    /// Produces canonical deterministic JSON bytes for this snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON encoding fails.
    pub fn canonical_json(&self) -> Result<Vec<u8>, SessionBundleError> {
        serde_json::to_vec(&self.0).map_err(SessionBundleError::Json)
    }
}

impl Serialize for WorkspaceSnapshot {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for WorkspaceSnapshot {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Value::deserialize(deserializer).map(Self)
    }
}

/// A typed event recorded in the append-only session journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "payload",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum JournalEvent {
    /// The active workspace changed.
    WorkspaceChanged(WorkspaceSnapshot),
    /// A view changed; details remain opaque to this persistence layer.
    ViewChanged(Value),
    /// A selection changed; details remain opaque to this persistence layer.
    SelectionChanged(Value),
    /// A reversible hypothesis or transform was applied.
    HypothesisApplied(Value),
    /// An analyst annotation was recorded.
    AnnotationAdded(Value),
}

/// One ordered journal record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JournalEntry {
    /// Monotonically increasing zero-based event sequence.
    pub sequence: u64,
    /// The typed event payload.
    pub event: JournalEvent,
}

/// A checked append-only sequence of session events.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Journal {
    entries: Vec<JournalEntry>,
}

impl Journal {
    /// Creates an empty journal.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Creates a journal after enforcing contiguous zero-based event sequences.
    ///
    /// # Errors
    ///
    /// Returns an error when an entry does not have the expected sequence number.
    pub fn from_entries(entries: Vec<JournalEntry>) -> Result<Self, SessionBundleError> {
        validate_event_order(&entries)?;
        Ok(Self { entries })
    }

    /// Returns all journal entries in order.
    #[must_use]
    pub fn entries(&self) -> &[JournalEntry] {
        &self.entries
    }

    /// Appends an event using the next contiguous sequence number.
    ///
    /// # Errors
    ///
    /// Returns an error if the next sequence number cannot be represented.
    pub fn append(&mut self, event: JournalEvent) -> Result<u64, SessionBundleError> {
        let sequence = u64::try_from(self.entries.len())
            .map_err(|_| SessionBundleError::NumericOverflow("journal sequence".to_owned()))?;
        self.entries.push(JournalEntry { sequence, event });
        Ok(sequence)
    }

    /// Serializes ordered entries as canonical newline-delimited JSON.
    ///
    /// # Errors
    ///
    /// Returns an error when ordering is invalid or JSON encoding fails.
    pub fn canonical_ndjson(&self) -> Result<Vec<u8>, SessionBundleError> {
        validate_event_order(&self.entries)?;
        let mut bytes = Vec::new();
        for entry in &self.entries {
            let line = serde_json::to_vec(entry).map_err(SessionBundleError::Json)?;
            bytes.extend_from_slice(&line);
            bytes.push(b'\n');
        }
        Ok(bytes)
    }

    fn from_ndjson(bytes: &[u8]) -> Result<Self, SessionBundleError> {
        if bytes.is_empty() {
            return Ok(Self::new());
        }
        if !bytes.ends_with(b"\n") {
            return Err(SessionBundleError::InvalidJournal(
                "journal must end with a newline".to_owned(),
            ));
        }
        let text = std::str::from_utf8(bytes).map_err(SessionBundleError::Utf8)?;
        let mut entries = Vec::new();
        for (line_index, line) in text.lines().enumerate() {
            if line.is_empty() {
                return Err(SessionBundleError::InvalidJournal(format!(
                    "journal line {} is empty",
                    line_index + 1
                )));
            }
            let entry = serde_json::from_str(line).map_err(SessionBundleError::Json)?;
            entries.push(entry);
        }
        Self::from_entries(entries)
    }
}

/// The persisted, source-free metadata for a session bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleManifest {
    schema: String,
    version: u32,
    source: SourceFingerprint,
    workspace: WorkspaceSnapshot,
    journal_sha256: String,
    journal_event_count: u64,
}

impl BundleManifest {
    /// Returns the bundle schema identifier.
    #[must_use]
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// Returns the bundle format version.
    #[must_use]
    pub const fn version(&self) -> u32 {
        self.version
    }

    /// Returns the source fingerprint, never source bytes or a source location.
    #[must_use]
    pub const fn source(&self) -> &SourceFingerprint {
        &self.source
    }

    /// Returns the opaque workspace state.
    #[must_use]
    pub const fn workspace(&self) -> &WorkspaceSnapshot {
        &self.workspace
    }

    /// Returns the SHA-256 digest of `journal.ndjson`.
    #[must_use]
    pub fn journal_sha256(&self) -> &str {
        &self.journal_sha256
    }

    /// Returns the number of ordered journal entries.
    #[must_use]
    pub const fn journal_event_count(&self) -> u64 {
        self.journal_event_count
    }

    fn validate(&self) -> Result<(), SessionBundleError> {
        if self.schema != BUNDLE_SCHEMA {
            return Err(SessionBundleError::UnsupportedSchema(self.schema.clone()));
        }
        if self.version != BUNDLE_VERSION {
            return Err(SessionBundleError::UnsupportedVersion(self.version));
        }
        if !is_sha256_hex(&self.journal_sha256) {
            return Err(SessionBundleError::InvalidManifest(
                "journal SHA-256 must be 64 lowercase hexadecimal characters".to_owned(),
            ));
        }
        Ok(())
    }
}

/// A complete source-free session bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBundle {
    manifest: BundleManifest,
    journal: Journal,
}

impl SessionBundle {
    /// Creates a new bundle with an integrity digest derived from the supplied journal.
    ///
    /// # Errors
    ///
    /// Returns an error when journal ordering or serialization is invalid.
    pub fn new(
        source: SourceFingerprint,
        workspace: WorkspaceSnapshot,
        journal: Journal,
    ) -> Result<Self, SessionBundleError> {
        let journal_bytes = journal.canonical_ndjson()?;
        let journal_event_count = u64::try_from(journal.entries.len())
            .map_err(|_| SessionBundleError::NumericOverflow("journal event count".to_owned()))?;
        let manifest = BundleManifest {
            schema: BUNDLE_SCHEMA.to_owned(),
            version: BUNDLE_VERSION,
            source,
            workspace,
            journal_sha256: sha256_hex(&journal_bytes),
            journal_event_count,
        };
        Ok(Self { manifest, journal })
    }

    /// Returns the persisted manifest.
    #[must_use]
    pub const fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    /// Returns the checked append-only journal.
    #[must_use]
    pub const fn journal(&self) -> &Journal {
        &self.journal
    }

    /// Saves `manifest.json` and `journal.ndjson` using same-directory atomic replacements.
    ///
    /// # Errors
    ///
    /// Returns an error when the bundle is internally inconsistent or filesystem persistence fails.
    pub fn save_to_directory(&self, directory: &Path) -> Result<(), SessionBundleError> {
        self.manifest.validate()?;
        let journal_bytes = self.journal.canonical_ndjson()?;
        let count = u64::try_from(self.journal.entries.len())
            .map_err(|_| SessionBundleError::NumericOverflow("journal event count".to_owned()))?;
        if self.manifest.journal_event_count != count {
            return Err(SessionBundleError::InvalidManifest(
                "journal event count does not match entries".to_owned(),
            ));
        }
        if self.manifest.journal_sha256 != sha256_hex(&journal_bytes) {
            return Err(SessionBundleError::JournalDigestMismatch);
        }
        fs::create_dir_all(directory).map_err(|source| SessionBundleError::Io {
            operation: "create bundle directory",
            source,
        })?;
        let manifest_bytes =
            serde_json::to_vec_pretty(&self.manifest).map_err(SessionBundleError::Json)?;
        atomic_write(&directory.join(MANIFEST_FILE), &manifest_bytes)?;
        atomic_write(&directory.join(JOURNAL_FILE), &journal_bytes)
    }

    /// Loads and validates a saved bundle, its journal digest, and its event ordering.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid schema/version, corruption, ordering failures, or I/O failures.
    pub fn load_from_directory(directory: &Path) -> Result<Self, SessionBundleError> {
        let manifest_path = directory.join(MANIFEST_FILE);
        let journal_path = directory.join(JOURNAL_FILE);
        let manifest_bytes = fs::read(&manifest_path).map_err(|source| SessionBundleError::Io {
            operation: "read manifest",
            source,
        })?;
        let manifest: BundleManifest =
            serde_json::from_slice(&manifest_bytes).map_err(SessionBundleError::Json)?;
        manifest.validate()?;
        let journal_bytes = fs::read(&journal_path).map_err(|source| SessionBundleError::Io {
            operation: "read journal",
            source,
        })?;
        if sha256_hex(&journal_bytes) != manifest.journal_sha256 {
            return Err(SessionBundleError::JournalDigestMismatch);
        }
        let journal = Journal::from_ndjson(&journal_bytes)?;
        let event_count = u64::try_from(journal.entries.len())
            .map_err(|_| SessionBundleError::NumericOverflow("journal event count".to_owned()))?;
        if event_count != manifest.journal_event_count {
            return Err(SessionBundleError::JournalEventCountMismatch {
                expected: manifest.journal_event_count,
                actual: event_count,
            });
        }
        Ok(Self { manifest, journal })
    }

    /// Checks whether the supplied bytes are the exact source represented by this bundle.
    #[must_use]
    pub fn reattach(&self, bytes: &[u8]) -> Reattachment {
        let actual_length = u64::try_from(bytes.len()).map_or(u64::MAX, |length| length);
        let actual_sha256 = sha256_hex(bytes);
        self.reattach_digest(actual_length, actual_sha256)
    }

    /// Checks a previously sealed whole-source digest without retaining source bytes.
    ///
    /// This is the large-source reattachment path: callers progressively hash
    /// the immutable candidate off the UI thread, then provide its stable
    /// length and canonical SHA-256.
    #[must_use]
    pub fn reattach_digest(
        &self,
        actual_length: u64,
        actual_sha256: impl Into<String>,
    ) -> Reattachment {
        let actual_sha256 = actual_sha256.into();
        if actual_length == self.manifest.source.byte_length
            && actual_sha256 == self.manifest.source.sha256
        {
            Reattachment::Match
        } else {
            Reattachment::Mismatch {
                expected_byte_length: self.manifest.source.byte_length,
                actual_byte_length: actual_length,
                expected_sha256: self.manifest.source.sha256.clone(),
                actual_sha256,
            }
        }
    }
}

/// The result of checking candidate bytes against a session source fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reattachment {
    /// Candidate bytes match both the saved length and SHA-256 digest.
    Match,
    /// Candidate bytes differ; only lengths and digests are reported.
    Mismatch {
        /// Saved source byte length.
        expected_byte_length: u64,
        /// Candidate source byte length.
        actual_byte_length: u64,
        /// Saved source digest.
        expected_sha256: String,
        /// Candidate source digest.
        actual_sha256: String,
    },
}

/// Errors returned while constructing, persisting, or validating a session bundle.
#[derive(Debug)]
pub enum SessionBundleError {
    /// A source fingerprint was malformed.
    InvalidFingerprint(String),
    /// The bundle manifest is internally malformed.
    InvalidManifest(String),
    /// A journal's event ordering or serialization is malformed.
    InvalidJournal(String),
    /// The bundle schema is not supported.
    UnsupportedSchema(String),
    /// The bundle format version is not supported.
    UnsupportedVersion(u32),
    /// The stored journal digest does not match its bytes.
    JournalDigestMismatch,
    /// The stored journal entry count does not match its contents.
    JournalEventCountMismatch {
        /// Entry count declared by the manifest.
        expected: u64,
        /// Entry count decoded from the journal.
        actual: u64,
    },
    /// A size could not fit into the on-disk representation.
    NumericOverflow(String),
    /// JSON encoding or decoding failed.
    Json(serde_json::Error),
    /// UTF-8 decoding failed.
    Utf8(std::str::Utf8Error),
    /// Filesystem I/O failed.
    Io {
        /// The attempted operation.
        operation: &'static str,
        /// The underlying I/O failure.
        source: io::Error,
    },
}

impl fmt::Display for SessionBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFingerprint(message) => {
                write!(formatter, "invalid source fingerprint: {message}")
            }
            Self::InvalidManifest(message) => {
                write!(formatter, "invalid session manifest: {message}")
            }
            Self::InvalidJournal(message) => {
                write!(formatter, "invalid session journal: {message}")
            }
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported session schema: {schema}")
            }
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported session version: {version}")
            }
            Self::JournalDigestMismatch => {
                write!(formatter, "session journal digest does not match")
            }
            Self::JournalEventCountMismatch { expected, actual } => {
                write!(
                    formatter,
                    "session journal count mismatch: expected {expected}, found {actual}"
                )
            }
            Self::NumericOverflow(subject) => write!(formatter, "numeric overflow for {subject}"),
            Self::Json(error) => write!(formatter, "session JSON error: {error}"),
            Self::Utf8(error) => write!(formatter, "session journal UTF-8 error: {error}"),
            Self::Io { operation, source } => write!(formatter, "failed to {operation}: {source}"),
        }
    }
}

impl std::error::Error for SessionBundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Utf8(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn validate_event_order(entries: &[JournalEntry]) -> Result<(), SessionBundleError> {
    for (index, entry) in entries.iter().enumerate() {
        let expected = u64::try_from(index)
            .map_err(|_| SessionBundleError::NumericOverflow("journal sequence".to_owned()))?;
        if entry.sequence != expected {
            return Err(SessionBundleError::InvalidJournal(format!(
                "event sequence {} must be {expected}, found {}",
                index, entry.sequence
            )));
        }
    }
    Ok(())
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), SessionBundleError> {
    let parent = destination.parent().ok_or_else(|| {
        SessionBundleError::InvalidManifest("bundle destination has no parent directory".to_owned())
    })?;
    let filename = destination.file_name().ok_or_else(|| {
        SessionBundleError::InvalidManifest("bundle destination has no file name".to_owned())
    })?;
    let filename = filename.to_string_lossy();
    let process_id = std::process::id();
    let mut temporary = None;
    for attempt in 0_u16..=255 {
        let candidate = parent.join(format!(".{filename}.{process_id}.{attempt}.tmp"));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(source) => {
                return Err(SessionBundleError::Io {
                    operation: "create temporary bundle file",
                    source,
                });
            }
        }
    }
    let (temporary_path, mut file) = temporary.ok_or_else(|| SessionBundleError::Io {
        operation: "allocate temporary bundle file",
        source: io::Error::new(
            io::ErrorKind::AlreadyExists,
            "temporary name collision limit reached",
        ),
    })?;
    let write_result = write_and_sync(&mut file, bytes);
    drop(file);
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }
    fs::rename(&temporary_path, destination).map_err(|source| SessionBundleError::Io {
        operation: "atomically replace bundle file",
        source,
    })
}

fn write_and_sync(file: &mut File, bytes: &[u8]) -> Result<(), SessionBundleError> {
    file.write_all(bytes)
        .map_err(|source| SessionBundleError::Io {
            operation: "write temporary bundle file",
            source,
        })?;
    file.sync_all().map_err(|source| SessionBundleError::Io {
        operation: "sync temporary bundle file",
        source,
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::{Value, json};

    use super::{
        Journal, JournalEntry, JournalEvent, Reattachment, SessionBundle, SessionBundleError,
        SourceFingerprint, WorkspaceSnapshot,
    };

    fn test_directory(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        std::env::temp_dir().join(format!("strata-session-{name}-{stamp}"))
    }

    fn source() -> Result<SourceFingerprint, SessionBundleError> {
        SourceFingerprint::from_bytes("sample.bin", b"source bytes are never persisted")
    }

    fn workspace() -> WorkspaceSnapshot {
        WorkspaceSnapshot::from_value(json!({"filters": {"kind": "xor"}, "zoom": 1.5}))
    }

    fn bundle() -> Result<SessionBundle, SessionBundleError> {
        let mut journal = Journal::new();
        journal.append(JournalEvent::ViewChanged(json!({"view": "regions"})))?;
        journal.append(JournalEvent::SelectionChanged(json!({"ranges": [[4, 9]]})))?;
        SessionBundle::new(source()?, workspace(), journal)
    }

    #[test]
    fn bundles_are_deterministic_and_source_free() -> Result<(), SessionBundleError> {
        let first = bundle()?;
        let second = bundle()?;
        let first_manifest =
            serde_json::to_vec(first.manifest()).map_err(SessionBundleError::Json)?;
        let second_manifest =
            serde_json::to_vec(second.manifest()).map_err(SessionBundleError::Json)?;
        let first_journal = first.journal().canonical_ndjson()?;
        let second_journal = second.journal().canonical_ndjson()?;

        assert_eq!(first_manifest, second_manifest);
        assert_eq!(first_journal, second_journal);
        let manifest_text = String::from_utf8(first_manifest).map_err(|error| {
            SessionBundleError::InvalidManifest(format!(
                "manifest UTF-8 conversion failed: {error}"
            ))
        })?;
        assert!(!manifest_text.contains("source bytes are never persisted"));
        assert!(!manifest_text.contains("/private/"));
        assert!(manifest_text.contains("sample.bin"));
        Ok(())
    }

    #[test]
    fn source_fingerprint_rejects_paths_and_preserves_no_source_bytes()
    -> Result<(), SessionBundleError> {
        assert!(matches!(
            SourceFingerprint::from_bytes("/private/source.bin", b"secret bytes"),
            Err(SessionBundleError::InvalidFingerprint(_))
        ));
        let directory = test_directory("source-free");
        let bundle = bundle()?;
        let result = (|| -> Result<(), SessionBundleError> {
            bundle.save_to_directory(&directory)?;
            let manifest = fs::read(directory.join("manifest.json")).map_err(|source| {
                SessionBundleError::Io {
                    operation: "read persisted manifest",
                    source,
                }
            })?;
            let journal = fs::read(directory.join("journal.ndjson")).map_err(|source| {
                SessionBundleError::Io {
                    operation: "read persisted journal",
                    source,
                }
            })?;
            let mut bundle_bytes = manifest;
            bundle_bytes.extend_from_slice(&journal);
            assert!(
                !bundle_bytes
                    .windows(b"source bytes are never persisted".len())
                    .any(|candidate| candidate == b"source bytes are never persisted")
            );
            Ok(())
        })();
        let _ = fs::remove_dir_all(&directory);
        result
    }

    #[test]
    fn save_load_round_trip_uses_manifest_and_journal_only() -> Result<(), SessionBundleError> {
        let directory = test_directory("round-trip");
        let original = bundle()?;
        let result = (|| -> Result<(), SessionBundleError> {
            original.save_to_directory(&directory)?;
            let loaded = SessionBundle::load_from_directory(&directory)?;
            assert_eq!(loaded, original);
            assert!(directory.join("manifest.json").is_file());
            assert!(directory.join("journal.ndjson").is_file());
            assert!(!directory.join("source.bin").exists());
            Ok(())
        })();
        let _ = fs::remove_dir_all(&directory);
        result
    }

    #[test]
    fn load_detects_tampered_journal() -> Result<(), SessionBundleError> {
        let directory = test_directory("tamper");
        let original = bundle()?;
        let result = (|| -> Result<(), SessionBundleError> {
            original.save_to_directory(&directory)?;
            fs::write(directory.join("journal.ndjson"), b"{\"sequence\":0}\n").map_err(
                |source| SessionBundleError::Io {
                    operation: "tamper journal",
                    source,
                },
            )?;
            let error = SessionBundle::load_from_directory(&directory)
                .err()
                .ok_or_else(|| SessionBundleError::InvalidJournal("tamper accepted".to_owned()))?;
            assert!(matches!(error, SessionBundleError::JournalDigestMismatch));
            Ok(())
        })();
        let _ = fs::remove_dir_all(&directory);
        result
    }

    #[test]
    fn load_rejects_unknown_schema_and_version() -> Result<(), SessionBundleError> {
        let directory = test_directory("schema-version");
        let original = bundle()?;
        let result = (|| -> Result<(), SessionBundleError> {
            original.save_to_directory(&directory)?;
            let manifest_path = directory.join("manifest.json");
            let manifest_text =
                fs::read_to_string(&manifest_path).map_err(|source| SessionBundleError::Io {
                    operation: "read manifest",
                    source,
                })?;
            fs::write(
                &manifest_path,
                manifest_text.replace("strata-session-bundle", "other-session-bundle"),
            )
            .map_err(|source| SessionBundleError::Io {
                operation: "rewrite schema manifest",
                source,
            })?;
            let schema_error = SessionBundle::load_from_directory(&directory)
                .err()
                .ok_or_else(|| SessionBundleError::InvalidManifest("schema accepted".to_owned()))?;
            assert!(matches!(
                schema_error,
                SessionBundleError::UnsupportedSchema(_)
            ));

            original.save_to_directory(&directory)?;
            let manifest_text =
                fs::read_to_string(&manifest_path).map_err(|source| SessionBundleError::Io {
                    operation: "read manifest",
                    source,
                })?;
            fs::write(
                manifest_path,
                manifest_text.replace("\"version\": 1", "\"version\": 2"),
            )
            .map_err(|source| SessionBundleError::Io {
                operation: "rewrite version manifest",
                source,
            })?;
            let version_error = SessionBundle::load_from_directory(&directory)
                .err()
                .ok_or_else(|| {
                    SessionBundleError::InvalidManifest("version accepted".to_owned())
                })?;
            assert!(matches!(
                version_error,
                SessionBundleError::UnsupportedVersion(2)
            ));
            Ok(())
        })();
        let _ = fs::remove_dir_all(&directory);
        result
    }

    #[test]
    fn load_rejects_non_contiguous_events_even_with_recomputed_digest()
    -> Result<(), SessionBundleError> {
        let directory = test_directory("ordering");
        let original = bundle()?;
        let result = (|| -> Result<(), SessionBundleError> {
            original.save_to_directory(&directory)?;
            let non_contiguous =
                b"{\"sequence\":2,\"event\":{\"type\":\"view_changed\",\"payload\":{}}}\n";
            let digest = super::sha256_hex(non_contiguous);
            fs::write(directory.join("journal.ndjson"), non_contiguous).map_err(|source| {
                SessionBundleError::Io {
                    operation: "rewrite journal",
                    source,
                }
            })?;
            let manifest_path = directory.join("manifest.json");
            let manifest_text =
                fs::read_to_string(&manifest_path).map_err(|source| SessionBundleError::Io {
                    operation: "read manifest",
                    source,
                })?;
            let replaced = manifest_text
                .replace(original.manifest().journal_sha256(), &digest)
                .replace("\"journal_event_count\": 2", "\"journal_event_count\": 1");
            fs::write(manifest_path, replaced).map_err(|source| SessionBundleError::Io {
                operation: "rewrite manifest",
                source,
            })?;
            let error = SessionBundle::load_from_directory(&directory)
                .err()
                .ok_or_else(|| {
                    SessionBundleError::InvalidJournal("ordering accepted".to_owned())
                })?;
            assert!(matches!(error, SessionBundleError::InvalidJournal(_)));
            Ok(())
        })();
        let _ = fs::remove_dir_all(&directory);
        result
    }

    #[test]
    fn reattachment_reports_match_and_typed_mismatch() -> Result<(), SessionBundleError> {
        let bundle = bundle()?;
        assert_eq!(
            bundle.reattach(b"source bytes are never persisted"),
            Reattachment::Match
        );
        assert_eq!(
            bundle.reattach_digest(32, super::sha256_hex(b"source bytes are never persisted")),
            Reattachment::Match
        );
        match bundle.reattach(b"different bytes") {
            Reattachment::Mismatch {
                expected_byte_length,
                actual_byte_length,
                expected_sha256,
                actual_sha256,
            } => {
                assert_eq!(expected_byte_length, 32);
                assert_eq!(actual_byte_length, 15);
                assert_ne!(expected_sha256, actual_sha256);
            }
            Reattachment::Match => {
                return Err(SessionBundleError::InvalidManifest(
                    "different bytes unexpectedly attached".to_owned(),
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn journal_requires_contiguous_zero_based_sequences() {
        let invalid = vec![JournalEntry {
            sequence: 1,
            event: JournalEvent::AnnotationAdded(json!({"note": "gap"})),
        }];
        assert!(matches!(
            Journal::from_entries(invalid),
            Err(SessionBundleError::InvalidJournal(_))
        ));
    }

    #[test]
    fn manifest_schema_tracks_the_runtime_contract() -> Result<(), SessionBundleError> {
        let schema: Value =
            serde_json::from_str(include_str!("../../../schemas/session-bundle.schema.json"))
                .map_err(SessionBundleError::Json)?;
        assert_eq!(
            schema
                .pointer("/properties/schema/const")
                .and_then(Value::as_str),
            Some("strata-session-bundle")
        );
        assert_eq!(
            schema
                .pointer("/properties/version/const")
                .and_then(Value::as_u64),
            Some(1)
        );
        let source_fields = schema
            .pointer("/$defs/sourceFingerprint/properties")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                SessionBundleError::InvalidManifest(
                    "bundle schema has no source fingerprint properties".to_owned(),
                )
            })?;
        assert!(source_fields.contains_key("alias"));
        assert!(source_fields.contains_key("byte_length"));
        assert!(source_fields.contains_key("sha256"));
        assert!(!source_fields.contains_key("path"));
        assert!(!source_fields.contains_key("bytes"));
        Ok(())
    }
}
