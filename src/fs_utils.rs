use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use glob::glob;
use serde::Serialize;

use crate::errors::AppError;

fn assert_readable_file(path: &Path) -> Result<(), AppError> {
    if !path.is_file() {
        return Err(AppError::MissingReadableFile(path.to_path_buf()));
    }

    Ok(())
}

fn has_glob_pattern(path: &Path) -> bool {
    let path = path.to_string_lossy();
    path.contains('*') || path.contains('?') || path.contains('[')
}

fn collect_json_directory_files(path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let mut paths = fs::read_dir(path)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|entry| entry.is_file())
        .filter(|entry| entry.extension().and_then(|value| value.to_str()) == Some("json"))
        .collect::<Vec<_>>();

    paths.sort();
    Ok(paths)
}

fn collect_json_glob_matches(path: &Path) -> Result<Vec<PathBuf>, AppError> {
    let pattern = path.to_string_lossy().into_owned();
    let mut paths = glob(&pattern)?
        .filter_map(Result::ok)
        .filter(|entry| entry.is_file())
        .collect::<Vec<_>>();

    paths.sort();
    Ok(paths)
}

pub fn resolve_json_input_paths<MissingErr, EmptyDirErr, EmptyGlobErr>(
    paths: &[PathBuf],
    missing_error: MissingErr,
    empty_directory_error: EmptyDirErr,
    empty_glob_error: EmptyGlobErr,
) -> Result<Vec<PathBuf>, AppError>
where
    MissingErr: Fn() -> AppError,
    EmptyDirErr: Fn(PathBuf) -> AppError,
    EmptyGlobErr: Fn(String) -> AppError,
{
    if paths.is_empty() {
        return Err(missing_error());
    }

    // Normalize supported path inputs into a stable, de-duplicated file list so repeated paths
    // and overlapping globs cannot trigger duplicate downstream work.
    let mut resolved_paths = BTreeSet::new();
    for path in paths {
        if path.is_dir() {
            let directory_files = collect_json_directory_files(path)?;
            if directory_files.is_empty() {
                return Err(empty_directory_error(path.to_path_buf()));
            }
            resolved_paths.extend(directory_files);
            continue;
        }

        if has_glob_pattern(path) {
            let glob_matches = collect_json_glob_matches(path)?;
            if glob_matches.is_empty() {
                return Err(empty_glob_error(path.to_string_lossy().into_owned()));
            }
            resolved_paths.extend(glob_matches);
            continue;
        }

        assert_readable_file(path)?;
        resolved_paths.insert(path.to_path_buf());
    }

    if resolved_paths.is_empty() {
        return Err(missing_error());
    }

    Ok(resolved_paths.into_iter().collect())
}

pub fn write_pretty_json_file<T: Serialize>(value: &T, path: &Path) -> Result<(), AppError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn resolve(paths: &[PathBuf]) -> Result<Vec<PathBuf>, AppError> {
        resolve_json_input_paths(
            paths,
            || AppError::MissingCaptureJobFiles,
            AppError::NoCaptureJobFilesFound,
            AppError::NoCaptureJobGlobMatches,
        )
    }

    #[test]
    fn resolve_json_input_paths_deduplicates_overlapping_inputs() {
        let temp = tempdir().expect("temp dir should be created");
        let fragments_dir = temp.path().join("fragments");
        fs::create_dir_all(&fragments_dir).expect("fragment directory should be created");
        let build = fragments_dir.join("build.json");
        fs::write(&build, "{}").expect("fragment should be written");

        let paths = resolve(&[build.clone(), fragments_dir.join("*.json")])
            .expect("path resolution should succeed");

        assert_eq!(paths, vec![build]);
    }

    #[test]
    fn write_pretty_json_file_creates_parent_directories() {
        let temp = tempdir().expect("temp dir should be created");
        let path = temp
            .path()
            .join("target")
            .join("actionspec")
            .join("payload.json");

        write_pretty_json_file(
            &serde_json::json!({ "run": { "workflow": "ci.yml" } }),
            &path,
        )
        .expect("json file should be written");

        let written = fs::read_to_string(path).expect("written file should be readable");
        assert!(written.contains("\"workflow\": \"ci.yml\""));
    }
}
