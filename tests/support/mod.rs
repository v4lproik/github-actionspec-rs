#![allow(dead_code)]
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use tempfile::TempDir;

pub fn write_declaration(repo_root: &Path, relative_path: &str, workflow: &str) {
    let path = repo_root.join(relative_path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "package actionspec\n\nworkflow: \"{workflow}\"\n\nrun: #Declaration.run & {{\n  workflow: workflow\n  jobs: {{\n    sample: {{\n      result: \"success\"\n    }}\n  }}\n}}\n"
        ),
    )
    .unwrap();
}

pub fn write_actual(path: &Path, workflow: &str) {
    fs::write(
        path,
        format!(
            "{{\"run\":{{\"workflow\":\"{workflow}\",\"jobs\":{{\"sample\":{{\"result\":\"success\"}}}}}}}}"
        ),
    )
    .unwrap();
}

pub fn install_fake_cue(temp_dir: &TempDir, mode: &str) -> HashMap<String, String> {
    let bin_dir = temp_dir.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let cue_path = bin_dir.join("cue");
    fs::write(
        &cue_path,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"version\" ]; then\n  exit 0\nfi\nif [ \"$1\" = \"vet\" ]; then\n  if [ \"{mode}\" = \"success\" ]; then\n    exit 0\n  fi\n  exit 9\nfi\nexit 1\n"
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&cue_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&cue_path, permissions).unwrap();
    }

    let mut env: HashMap<String, String> = std::env::vars().collect();
    env.insert(
        "PATH".to_owned(),
        format!("{}:{}", bin_dir.display(), env.get("PATH").cloned().unwrap_or_default()),
    );
    env
}
