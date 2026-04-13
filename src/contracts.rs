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
