use std::path::{Path, PathBuf};

pub const DEFAULT_DECLARATIONS_DIR: &str = ".github/actionspec";

pub fn package_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn workflow_schema_path() -> PathBuf {
    package_root().join("schema/workflow_run.cue")
}

pub fn declaration_schema_path() -> PathBuf {
    package_root().join("schema/declaration.cue")
}

pub fn resolve_declarations_dir(repo_root: &Path, declarations_dir: &Path) -> PathBuf {
    repo_root.join(declarations_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_root_points_to_the_repo_root() {
        assert!(package_root().join("Cargo.toml").is_file());
    }

    #[test]
    fn bundled_schema_paths_point_to_existing_files() {
        assert!(workflow_schema_path().is_file());
        assert!(declaration_schema_path().is_file());
    }

    #[test]
    fn resolve_declarations_dir_joins_repo_and_relative_dir() {
        let repo_root = Path::new("/tmp/demo-repo");
        let declarations_dir = Path::new(".github/actionspec");

        assert_eq!(
            resolve_declarations_dir(repo_root, declarations_dir),
            PathBuf::from("/tmp/demo-repo/.github/actionspec"),
        );
    }
}
