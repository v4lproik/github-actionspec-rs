use clap::Parser;
use github_actionspec_rs::bootstrap::{bootstrap_repo_workflow, BootstrapOptions};
use github_actionspec_rs::capture::{
    capture_workflow_run, emit_job_fragment, write_captured_workflow_run,
    write_emitted_job_fragment, CaptureWorkflowOptions, EmitFragmentOptions,
};
use github_actionspec_rs::cli::{Cli, Command};
use github_actionspec_rs::dashboard::{load_validation_report, write_dashboard_markdown};
use github_actionspec_rs::discovery::discover_declarations;
use github_actionspec_rs::types::ValidationStatus;
use github_actionspec_rs::validate::{
    validate_contract, validate_repo_workflow, write_validation_report, ValidateContractOptions,
    ValidateRepoWorkflowOptions,
};
use github_actionspec_rs::workflow_calls::{
    validate_workflow_callers, write_workflow_call_report, ValidateCallersOptions,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn normalize_path_inputs(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut normalized = Vec::new();

    for path in paths {
        // GitHub Action inputs frequently arrive as newline-delimited path lists, so normalize
        // them once before dispatching to the command-specific logic.
        let path = path.to_string_lossy();
        normalized.extend(
            path.lines()
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(PathBuf::from),
        );
    }

    normalized
}

fn summarize_validation_failures(report: &github_actionspec_rs::types::ValidationReport) -> String {
    report
        .actuals
        .iter()
        .filter(|actual| actual.status == ValidationStatus::Failed)
        .map(|actual| {
            let details = if actual.issues.is_empty() {
                actual
                    .error
                    .as_deref()
                    .unwrap_or("validation failed without a reported cue error")
                    .to_owned()
            } else {
                actual
                    .issues
                    .iter()
                    .map(github_actionspec_rs::types::ValidationIssue::detail_label)
                    .collect::<Vec<_>>()
                    .join("; ")
            };
            format!("- {}: {}", actual.actual_path.display(), details)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn summarize_workflow_call_issues(
    issues: &[github_actionspec_rs::workflow_calls::CallerValidationIssue],
) -> String {
    issues
        .iter()
        .map(|issue| {
            format!(
                "- {} job `{}` -> {}: {}",
                issue.caller_workflow.display(),
                issue.job_id,
                issue.callee_workflow.display(),
                issue.message
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn run() -> Result<(), github_actionspec_rs::errors::AppError> {
    let cli = Cli::parse();

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
            let fragment = emit_job_fragment(EmitFragmentOptions {
                job,
                result,
                outputs: output,
                matrix,
                step_conclusions: step_conclusion,
                step_outputs: step_output,
            })?;
            write_emitted_job_fragment(&fragment, &file)?;
        }
        Command::Capture {
            workflow,
            ref_name,
            input,
            job_file,
            output,
        } => {
            let envelope = capture_workflow_run(CaptureWorkflowOptions {
                workflow,
                ref_name,
                inputs: input,
                job_files: normalize_path_inputs(job_file),
            })?;
            write_captured_workflow_run(&envelope, &output)?;
        }
        Command::Validate {
            schema,
            contract,
            actual,
        } => {
            validate_contract(ValidateContractOptions {
                schema_paths: schema,
                contract_path: contract,
                actual_paths: normalize_path_inputs(actual),
                cwd: None,
                env: None,
            })?;
        }
        Command::Discover {
            repo,
            declarations_dir,
        } => {
            let declarations = discover_declarations(&repo, &declarations_dir)?;
            println!("{}", serde_json::to_string_pretty(&declarations)?);
        }
        Command::Bootstrap {
            repo,
            workflow,
            actual,
            declarations_dir,
            workflows_dir,
            fixtures_dir,
            force,
        } => {
            let result = bootstrap_repo_workflow(BootstrapOptions {
                repo_root: repo,
                workflow,
                actual,
                declarations_dir,
                workflows_dir,
                fixtures_dir,
                force,
            })?;
            println!("Created starter workflow contract artifacts:");
            println!("- workflow: {}", result.workflow);
            println!("- workflow file: {}", result.workflow_path.display());
            println!("- declaration: {}", result.declaration_path.display());
            println!("- baseline: {}", result.actual_path.display());
            println!("- ci snippet: {}", result.snippet_path.display());
            println!(
                "- seeded from actual: {}",
                if result.seeded_from_actual {
                    "yes"
                } else {
                    "no"
                }
            );
            println!(
                "Next: just validate-repo-dashboard . {} {} target/actionspec/validation-report.json target/actionspec/dashboard.md",
                result.workflow,
                result.actual_path.display(),
            );
        }
        Command::ValidateCallers {
            repo,
            workflows_dir,
            report_file,
            dry_run,
        } => {
            let result = validate_workflow_callers(ValidateCallersOptions {
                repo_root: repo,
                workflows_dir,
            })?;
            if let Some(report_file) = report_file {
                write_workflow_call_report(&result.report, &report_file)?;
            }
            if result.failed_count > 0 && !dry_run {
                return Err(
                    github_actionspec_rs::errors::AppError::WorkflowCallerValidationFailures {
                        failed: result.failed_count,
                        details: summarize_workflow_call_issues(&result.report.issues),
                    },
                );
            }
        }
        Command::ValidateRepo {
            repo,
            workflow,
            actual,
            declarations_dir,
            report_file,
            dry_run,
        } => {
            let result = validate_repo_workflow(ValidateRepoWorkflowOptions {
                repo_root: repo,
                workflow,
                actual_paths: normalize_path_inputs(actual),
                declarations_dir,
                cwd: None,
                env: None,
            })?;
            if let Some(report_file) = report_file {
                write_validation_report(&result.report, &report_file)?;
            }
            if result.failed_count > 0 && !dry_run {
                return Err(github_actionspec_rs::errors::AppError::ValidationFailures {
                    failed: result.failed_count,
                    total: result.report.actuals.len(),
                    details: summarize_validation_failures(&result.report),
                });
            }
        }
        Command::Dashboard {
            current,
            baseline,
            output_key,
            output,
        } => {
            let current = load_validation_report(&current)?;
            let baseline = match baseline {
                Some(path) => Some(load_validation_report(&path)?),
                None => None,
            };
            let output_keys =
                (!output_key.is_empty()).then(|| output_key.into_iter().collect::<BTreeSet<_>>());
            write_dashboard_markdown(&current, baseline.as_ref(), output_keys.as_ref(), &output)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_path_inputs;
    use github_actionspec_rs::bootstrap::{default_fixtures_dir, default_workflows_dir};
    use std::path::PathBuf;

    #[test]
    fn normalize_path_inputs_splits_newline_separated_values() {
        let actuals = normalize_path_inputs(vec![PathBuf::from(
            "fixtures/one.json\n\n fixtures/two.json \n",
        )]);

        assert_eq!(
            actuals,
            vec![
                PathBuf::from("fixtures/one.json"),
                PathBuf::from("fixtures/two.json"),
            ]
        );
    }

    #[test]
    fn bootstrap_defaults_match_repo_layout() {
        assert_eq!(default_workflows_dir(), PathBuf::from(".github/workflows"));
        assert_eq!(default_fixtures_dir(), PathBuf::from("tests/fixtures"));
    }
}
