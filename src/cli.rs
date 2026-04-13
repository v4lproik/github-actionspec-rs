use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(name = "github-actionspec")]
#[command(about = "Validate GitHub Actions workflow contracts expressed in CUE.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Validate {
        #[arg(long = "schema", required = true)]
        schema: Vec<PathBuf>,
        #[arg(long)]
        contract: PathBuf,
        #[arg(long, required = true)]
        actual: Vec<PathBuf>,
    },
    Discover {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long = "declarations-dir", default_value = ".github/actionspec")]
        declarations_dir: PathBuf,
    },
    ValidateRepo {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        workflow: Option<String>,
        #[arg(long, required = true)]
        actual: Vec<PathBuf>,
        #[arg(long = "declarations-dir", default_value = ".github/actionspec")]
        declarations_dir: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_validate_command_with_multiple_schemas() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "validate",
            "--schema",
            "schema/workflow_run.cue",
            "--schema",
            "schema/declaration.cue",
            "--contract",
            "contract.cue",
            "--actual",
            "actual.json",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::Validate {
                schema,
                contract,
                actual,
            } => {
                assert_eq!(schema.len(), 2);
                assert_eq!(schema[0], PathBuf::from("schema/workflow_run.cue"));
                assert_eq!(schema[1], PathBuf::from("schema/declaration.cue"));
                assert_eq!(contract, PathBuf::from("contract.cue"));
                assert_eq!(actual, vec![PathBuf::from("actual.json")]);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_validate_command_with_multiple_actuals() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "validate",
            "--schema",
            "schema/workflow_run.cue",
            "--contract",
            "contract.cue",
            "--actual",
            "actual-one.json",
            "--actual",
            "actual-two.json",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::Validate {
                actual, contract, ..
            } => {
                assert_eq!(contract, PathBuf::from("contract.cue"));
                assert_eq!(
                    actual,
                    vec![
                        PathBuf::from("actual-one.json"),
                        PathBuf::from("actual-two.json")
                    ]
                );
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn discover_defaults_to_github_actionspec_directory() {
        let cli = Cli::try_parse_from(["github-actionspec", "discover", "--repo", "/tmp/repo"])
            .expect("cli should parse");

        match cli.command {
            Command::Discover {
                repo,
                declarations_dir,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(declarations_dir, PathBuf::from(".github/actionspec"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn validate_repo_defaults_to_github_actionspec_directory() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "validate-repo",
            "--repo",
            "/tmp/repo",
            "--workflow",
            "ci.yml",
            "--actual",
            "actual.json",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::ValidateRepo {
                repo,
                workflow,
                actual,
                declarations_dir,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(workflow, Some("ci.yml".to_owned()));
                assert_eq!(actual, vec![PathBuf::from("actual.json")]);
                assert_eq!(declarations_dir, PathBuf::from(".github/actionspec"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn validate_repo_accepts_missing_workflow_for_inference() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "validate-repo",
            "--repo",
            "/tmp/repo",
            "--actual",
            "actual.json",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::ValidateRepo {
                repo,
                workflow,
                actual,
                declarations_dir,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(workflow, None);
                assert_eq!(actual, vec![PathBuf::from("actual.json")]);
                assert_eq!(declarations_dir, PathBuf::from(".github/actionspec"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
