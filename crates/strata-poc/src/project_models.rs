//! Versioned local project locators and persisted project preferences.
#![allow(clippy::redundant_pub_crate)]

use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub(crate) const LOCAL_PROJECT_SUFFIX: &str = ".strata-project";
pub(crate) const LOCAL_PROJECT_VERSION: u32 = 1;
const LOCAL_PROJECT_KIND: &str = "strata_local_project";
const MAX_LOCAL_PROJECT_BYTES: u64 = 64 * 1024;
const MAX_PREFERENCES_BYTES: u64 = 64 * 1024;
const MAX_LOCATOR_BYTES: usize = 8 * 1024;

/// A local convenience locator. The referenced session remains source-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalProjectFile {
    kind: String,
    version: u32,
    pub(crate) session_bundle: String,
    pub(crate) source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) signature_pack: Option<LocalSignaturePack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LocalSignaturePack {
    pub(crate) path: String,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ProjectPreferences {
    pub(crate) version: u32,
    pub(crate) reopen_last_project: bool,
    pub(crate) last_project_path: String,
    pub(crate) default_signature_pack_path: String,
}

impl Default for ProjectPreferences {
    fn default() -> Self {
        Self {
            version: LOCAL_PROJECT_VERSION,
            reopen_last_project: false,
            last_project_path: String::new(),
            default_signature_pack_path: String::new(),
        }
    }
}

impl LocalProjectFile {
    pub(crate) fn new(
        project_path: &Path,
        session_bundle: &Path,
        source: &Path,
        signature_pack: Option<(&Path, &str)>,
    ) -> Result<Self, String> {
        let project_path = absolutized_path(project_path)?;
        let session_bundle = absolutized_path(session_bundle)?;
        let source = absolutized_path(source)?;
        let base = project_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let signature_pack = if let Some((path, sha256)) = signature_pack {
            let path = absolutized_path(path)?;
            Some(LocalSignaturePack {
                path: locator_from_path(&path, base)?,
                sha256: sha256.to_owned(),
            })
        } else {
            None
        };
        let project = Self {
            kind: LOCAL_PROJECT_KIND.to_owned(),
            version: LOCAL_PROJECT_VERSION,
            session_bundle: locator_from_path(&session_bundle, base)?,
            source: locator_from_path(&source, base)?,
            signature_pack,
        };
        project.validate()?;
        Ok(project)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.kind != LOCAL_PROJECT_KIND {
            return Err("not a Strata local project".to_owned());
        }
        if self.version != LOCAL_PROJECT_VERSION {
            return Err(format!(
                "unsupported local project version {}",
                self.version
            ));
        }
        validate_locator(&self.session_bundle, "session bundle")?;
        validate_locator(&self.source, "source")?;
        if let Some(signature_pack) = &self.signature_pack {
            validate_locator(&signature_pack.path, "signature pack")?;
            if signature_pack.sha256.len() != 64
                || !signature_pack
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_hexdigit())
            {
                return Err("signature-pack SHA-256 must contain 64 hexadecimal digits".to_owned());
            }
        }
        Ok(())
    }

    pub(crate) fn resolved_session_bundle(&self, project_path: &Path) -> PathBuf {
        resolve_locator(project_path, &self.session_bundle)
    }

    pub(crate) fn resolved_source(&self, project_path: &Path) -> PathBuf {
        resolve_locator(project_path, &self.source)
    }

    pub(crate) fn resolved_signature_pack(&self, project_path: &Path) -> Option<PathBuf> {
        self.signature_pack
            .as_ref()
            .map(|signature_pack| resolve_locator(project_path, &signature_pack.path))
    }
}

impl ProjectPreferences {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.version != LOCAL_PROJECT_VERSION {
            return Err(format!(
                "unsupported project preferences version {}",
                self.version
            ));
        }
        validate_optional_locator(&self.last_project_path, "last project")?;
        validate_optional_locator(&self.default_signature_pack_path, "default signature pack")?;
        if self.reopen_last_project && self.last_project_path.trim().is_empty() {
            return Err("reopen-last-project requires a project path".to_owned());
        }
        Ok(())
    }
}

pub(crate) fn load_local_project(path: &Path) -> Result<LocalProjectFile, String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("cannot inspect local project {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("local project is not a file: {}", path.display()));
    }
    if metadata.len() > MAX_LOCAL_PROJECT_BYTES {
        return Err(format!(
            "local project is {} bytes; maximum is {MAX_LOCAL_PROJECT_BYTES}",
            metadata.len()
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read local project {}: {error}", path.display()))?;
    let project: LocalProjectFile = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid local project {}: {error}", path.display()))?;
    project.validate()?;
    Ok(project)
}

pub(crate) fn save_local_project(path: &Path, project: &LocalProjectFile) -> Result<(), String> {
    project.validate()?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        return Err(format!(
            "local project directory does not exist: {}",
            parent.display()
        ));
    }
    let bytes = serde_json::to_vec_pretty(project)
        .map_err(|error| format!("cannot encode local project: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_LOCAL_PROJECT_BYTES {
        return Err("encoded local project exceeds its 64 KiB bound".to_owned());
    }
    write_atomic(path, &bytes, "local project")
}

pub(crate) fn default_project_preferences_path() -> PathBuf {
    if let Some(path) = std::env::var_os("STRATA_PREFERENCES_PATH") {
        return PathBuf::from(path);
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Strata")
            .join("project-preferences.json");
    }
    std::env::temp_dir()
        .join("Strata")
        .join("project-preferences.json")
}

pub(crate) fn load_project_preferences_file(path: &Path) -> Result<ProjectPreferences, String> {
    if !path.exists() {
        return Ok(ProjectPreferences::default());
    }
    let metadata = std::fs::metadata(path).map_err(|error| {
        format!(
            "cannot inspect project preferences {}: {error}",
            path.display()
        )
    })?;
    if !metadata.is_file() || metadata.len() > MAX_PREFERENCES_BYTES {
        return Err("project preferences are not a bounded regular file".to_owned());
    }
    let bytes = std::fs::read(path).map_err(|error| {
        format!(
            "cannot read project preferences {}: {error}",
            path.display()
        )
    })?;
    let preferences: ProjectPreferences = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid project preferences: {error}"))?;
    preferences.validate()?;
    Ok(preferences)
}

pub(crate) fn save_project_preferences_file(
    path: &Path,
    preferences: &ProjectPreferences,
) -> Result<(), String> {
    preferences.validate()?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "cannot create project preferences directory {}: {error}",
            parent.display()
        )
    })?;
    let bytes = serde_json::to_vec_pretty(preferences)
        .map_err(|error| format!("cannot encode project preferences: {error}"))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_PREFERENCES_BYTES {
        return Err("encoded project preferences exceed their 64 KiB bound".to_owned());
    }
    write_atomic(path, &bytes, "project preferences")
}

fn write_atomic(path: &Path, bytes: &[u8], label: &str) -> Result<(), String> {
    let temporary = temporary_project_path(path)?;
    let write_result = (|| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|error| {
                format!(
                    "cannot create temporary project {}: {error}",
                    temporary.display()
                )
            })?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("cannot persist {label}: {error}"))?;
        std::fs::rename(&temporary, path).map_err(|error| {
            format!(
                "cannot replace {label} {} atomically: {error}",
                path.display()
            )
        })
    })();
    if write_result.is_err() && temporary.is_file() {
        let _ = std::fs::remove_file(&temporary);
    }
    write_result
}

pub(crate) fn is_local_project_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(LOCAL_PROJECT_SUFFIX))
}

pub(crate) fn normalized_project_path(path: &Path) -> PathBuf {
    if is_local_project_path(path) {
        return path.to_owned();
    }
    let mut suffixed = path.as_os_str().to_owned();
    suffixed.push(LOCAL_PROJECT_SUFFIX);
    PathBuf::from(suffixed)
}

pub(crate) fn absolutized_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_owned());
    }
    std::env::current_dir()
        .map(|current| current.join(path))
        .map_err(|error| format!("cannot resolve local path {}: {error}", path.display()))
}

pub(crate) fn derived_session_path(project_path: &Path) -> Result<PathBuf, String> {
    let normalized = normalized_project_path(project_path);
    let file_name = normalized
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "local project filename must be valid UTF-8".to_owned())?;
    let stem = file_name
        .strip_suffix(LOCAL_PROJECT_SUFFIX)
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| "local project filename is missing its project name".to_owned())?;
    Ok(normalized.with_file_name(format!("{stem}.strata-session")))
}

fn locator_from_path(path: &Path, base: &Path) -> Result<String, String> {
    let locator = path.strip_prefix(base).unwrap_or(path);
    let locator = locator
        .to_str()
        .ok_or_else(|| format!("project locator is not valid UTF-8: {}", path.display()))?;
    validate_locator(locator, "project")?;
    Ok(locator.to_owned())
}

fn resolve_locator(project_path: &Path, locator: &str) -> PathBuf {
    let locator = PathBuf::from(locator);
    if locator.is_absolute() {
        return locator;
    }
    project_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(locator)
}

fn validate_optional_locator(locator: &str, label: &str) -> Result<(), String> {
    if locator.trim().is_empty() {
        return Ok(());
    }
    validate_locator(locator, label)
}

fn validate_locator(locator: &str, label: &str) -> Result<(), String> {
    if locator.trim().is_empty()
        || locator.len() > MAX_LOCATOR_BYTES
        || locator.contains('\0')
        || locator.contains(['\r', '\n'])
    {
        return Err(format!("{label} locator is empty, oversized, or unsafe"));
    }
    Ok(())
}

fn temporary_project_path(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "local project filename must be valid UTF-8".to_owned())?;
    Ok(path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id())))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "strata-{label}-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ))
    }

    #[test]
    fn project_round_trip_resolves_relative_session() -> Result<(), String> {
        let directory = unique_path("project-model");
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let project_path = directory.join("firmware.strata-project");
        let session_path = directory.join("firmware.strata-session");
        let source_path = Path::new("/usr/bin/file");
        let signature_path = Path::new("/tmp/signatures.json");
        let digest = "a".repeat(64);
        let project = LocalProjectFile::new(
            &project_path,
            &session_path,
            source_path,
            Some((signature_path, &digest)),
        )?;
        save_local_project(&project_path, &project)?;
        let restored = load_local_project(&project_path)?;
        assert_eq!(
            restored.resolved_session_bundle(&project_path),
            session_path
        );
        assert_eq!(restored.resolved_source(&project_path), source_path);
        assert_eq!(
            restored.resolved_signature_pack(&project_path).as_deref(),
            Some(signature_path)
        );
        std::fs::remove_file(&project_path).map_err(|error| error.to_string())?;
        std::fs::remove_dir(&directory).map_err(|error| error.to_string())?;
        Ok(())
    }

    #[test]
    fn derived_session_path_is_stable() -> Result<(), String> {
        let project = Path::new("/tmp/research.strata-project");
        assert_eq!(
            derived_session_path(project)?,
            Path::new("/tmp/research.strata-session")
        );
        assert_eq!(normalized_project_path(Path::new("/tmp/research")), project);
        Ok(())
    }

    #[test]
    fn invalid_signature_digest_is_rejected() -> Result<(), String> {
        let project = LocalProjectFile::new(
            Path::new("/tmp/test.strata-project"),
            Path::new("/tmp/test.strata-session"),
            Path::new("/usr/bin/file"),
            None,
        )?;
        let mut invalid = project;
        invalid.signature_pack = Some(LocalSignaturePack {
            path: "/tmp/signatures.json".to_owned(),
            sha256: "xyz".to_owned(),
        });
        assert!(invalid.validate().is_err());
        Ok(())
    }

    #[test]
    fn preferences_round_trip_as_bounded_json() -> Result<(), String> {
        let directory = unique_path("project-preferences");
        let path = directory.join("preferences.json");
        let preferences = ProjectPreferences {
            reopen_last_project: true,
            last_project_path: "/tmp/research.strata-project".to_owned(),
            default_signature_pack_path: "/tmp/signatures.json".to_owned(),
            ..ProjectPreferences::default()
        };
        save_project_preferences_file(&path, &preferences)?;
        assert_eq!(load_project_preferences_file(&path)?, preferences);
        std::fs::remove_file(path).map_err(|error| error.to_string())?;
        std::fs::remove_dir(directory).map_err(|error| error.to_string())?;
        Ok(())
    }
}
