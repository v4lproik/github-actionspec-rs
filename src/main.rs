use clap::Parser;
use github_actionspec_rs::cli::{Cli, Command};
use github_actionspec_rs::dashboard::{load_validation_report, write_dashboard_markdown};
use github_actionspec_rs::discovery::discover_declarations;
use github_actionspec_rs::types::ValidationStatus;
use github_actionspec_rs::validate::{
    validate_contract, validate_repo_workflow, write_validation_report, ValidateContractOptions,
    ValidateRepoWorkflowOptions,
};
use std::collections::BTreeSet;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn normalize_actual_inputs(actuals: Vec<PathBuf>) -> Vec<PathBuf> {
    actuals
        .into_iter()
        .flat_map(|actual| {
            // The GitHub Action accepts newline-delimited payload inputs, so normalize them before
            // handing control to the validation layer.
            let actual = actual.to_string_lossy();
            actual
                .lines()
                .map(str::trim)
                .filter(|segment| !segment.is_empty())
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn summarize_validation_failures(report: &github_actionspec_rs::types::ValidationReport) -> String {
    report
        .actuals
        .iter()
        .filter(|actual| actual.status == ValidationStatus::Failed)
        .map(|actual| {
            format!(
                "- {}: {}",
                actual.actual_path.display(),
                actual
                    .error
                    .as_deref()
                    .unwrap_or("validation failed without a reported cue error")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn run() -> Result<(), github_actionspec_rs::errors::AppError> {
    let cli = Cli::parse();

    match cli.command {
        Command::Validate {
            schema,
            contract,
            actual,
        } => {
            validate_contract(ValidateContractOptions {
                schema_paths: schema,
                contract_path: contract,
                actual_paths: normalize_actual_inputs(actual),
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
        Command::ValidateRepo {
            repo,
            workflow,
            actual,
            declarations_dir,
            report_file,
        } => {
            let result = validate_repo_workflow(ValidateRepoWorkflowOptions {
                repo_root: repo,
                workflow,
                actual_paths: normalize_actual_inputs(actual),
                declarations_dir,
                report_file: report_file.clone(),
                cwd: None,
                env: None,
            })?;
            if let Some(report_file) = report_file {
                write_validation_report(&result.report, &report_file)?;
            }
            if result.failed_count > 0 {
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
    use super::normalize_actual_inputs;
    use std::path::PathBuf;

    #[test]
    fn normalize_actual_inputs_splits_newline_separated_values() {
        let actuals = normalize_actual_inputs(vec![PathBuf::from(
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
}
