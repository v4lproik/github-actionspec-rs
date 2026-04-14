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
    EmitFragment {
        #[arg(long)]
        job: String,
        #[arg(long)]
        result: String,
        #[arg(long = "output")]
        output: Vec<String>,
        #[arg(long = "matrix")]
        matrix: Vec<String>,
        #[arg(long = "step-conclusion")]
        step_conclusion: Vec<String>,
        #[arg(long = "step-output")]
        step_output: Vec<String>,
        #[arg(long = "file")]
        file: PathBuf,
    },
    Capture {
        #[arg(long)]
        workflow: String,
        #[arg(long = "ref")]
        ref_name: Option<String>,
        #[arg(long = "input")]
        input: Vec<String>,
        #[arg(long = "job-file", required = true)]
        job_file: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
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
    ValidateCallers {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long = "workflows-dir", default_value = ".github/workflows")]
        workflows_dir: PathBuf,
        #[arg(long = "report-file")]
        report_file: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
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
        #[arg(long = "report-file")]
        report_file: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Dashboard {
        #[arg(long)]
        current: PathBuf,
        #[arg(long)]
        baseline: Option<PathBuf>,
        #[arg(long = "output-key")]
        output_key: Vec<String>,
        #[arg(long)]
        output: PathBuf,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_emit_fragment_command() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "emit-fragment",
            "--job",
            "build",
            "--result",
            "success",
            "--output",
            "artifact=build-ts-service",
            "--matrix",
            "app=build-ts-service",
            "--step-conclusion",
            "compile=success",
            "--step-output",
            "compile.digest=sha256:abc123",
            "--file",
            "fragments/build.json",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::EmitFragment {
                job,
                result,
                output,
                matrix,
                step_conclusion,
                step_output,
                file,
            } => {
                assert_eq!(job, "build");
                assert_eq!(result, "success");
                assert_eq!(output, vec!["artifact=build-ts-service".to_owned()]);
                assert_eq!(matrix, vec!["app=build-ts-service".to_owned()]);
                assert_eq!(step_conclusion, vec!["compile=success".to_owned()]);
                assert_eq!(step_output, vec!["compile.digest=sha256:abc123".to_owned()]);
                assert_eq!(file, PathBuf::from("fragments/build.json"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_capture_command() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "capture",
            "--workflow",
            "ci.yml",
            "--ref",
            "main",
            "--input",
            "run_ci=true",
            "--job-file",
            "fragments",
            "--job-file",
            "more/*.json",
            "--output",
            "actual.json",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::Capture {
                workflow,
                ref_name,
                input,
                job_file,
                output,
            } => {
                assert_eq!(workflow, "ci.yml");
                assert_eq!(ref_name, Some("main".to_owned()));
                assert_eq!(input, vec!["run_ci=true".to_owned()]);
                assert_eq!(
                    job_file,
                    vec![PathBuf::from("fragments"), PathBuf::from("more/*.json")]
                );
                assert_eq!(output, PathBuf::from("actual.json"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

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
                report_file,
                dry_run,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(workflow, Some("ci.yml".to_owned()));
                assert_eq!(actual, vec![PathBuf::from("actual.json")]);
                assert_eq!(declarations_dir, PathBuf::from(".github/actionspec"));
                assert_eq!(report_file, None);
                assert!(!dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn validate_callers_defaults_to_github_workflows_directory() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "validate-callers",
            "--repo",
            "/tmp/repo",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::ValidateCallers {
                repo,
                workflows_dir,
                report_file,
                dry_run,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(workflows_dir, PathBuf::from(".github/workflows"));
                assert_eq!(report_file, None);
                assert!(!dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn validate_callers_accepts_report_file_and_dry_run() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "validate-callers",
            "--repo",
            "/tmp/repo",
            "--report-file",
            "callers.json",
            "--dry-run",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::ValidateCallers {
                repo,
                workflows_dir,
                report_file,
                dry_run,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(workflows_dir, PathBuf::from(".github/workflows"));
                assert_eq!(report_file, Some(PathBuf::from("callers.json")));
                assert!(dry_run);
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
                report_file,
                dry_run,
            } => {
                assert_eq!(repo, PathBuf::from("/tmp/repo"));
                assert_eq!(workflow, None);
                assert_eq!(actual, vec![PathBuf::from("actual.json")]);
                assert_eq!(declarations_dir, PathBuf::from(".github/actionspec"));
                assert_eq!(report_file, None);
                assert!(!dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_dashboard_command() {
        let cli = Cli::try_parse_from([
            "github-actionspec",
            "dashboard",
            "--current",
            "current.json",
            "--baseline",
            "baseline.json",
            "--output-key",
            "contract_build",
            "--output-key",
            "artifact_name",
            "--output",
            "dashboard.md",
        ])
        .expect("cli should parse");

        match cli.command {
            Command::Dashboard {
                current,
                baseline,
                output_key,
                output,
            } => {
                assert_eq!(current, PathBuf::from("current.json"));
                assert_eq!(baseline, Some(PathBuf::from("baseline.json")));
                assert_eq!(output_key, vec!["contract_build", "artifact_name"]);
                assert_eq!(output, PathBuf::from("dashboard.md"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
