use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::errors::AppError;
use crate::types::{ActualValidationReport, ValidationReport, ValidationStatus};

fn count_status(actuals: &[ActualValidationReport], status: ValidationStatus) -> usize {
    actuals
        .iter()
        .filter(|actual| actual.status == status)
        .count()
}

fn collect_job_names(
    current: &ValidationReport,
    baseline: Option<&ValidationReport>,
) -> Vec<String> {
    let mut job_names = BTreeSet::new();
    for actual in &current.actuals {
        job_names.extend(actual.jobs.keys().cloned());
    }
    if let Some(baseline) = baseline {
        for actual in &baseline.actuals {
            job_names.extend(actual.jobs.keys().cloned());
        }
    }
    job_names.into_iter().collect()
}

fn compare_actual(
    current: &ActualValidationReport,
    baseline: Option<&ActualValidationReport>,
) -> String {
    let Some(baseline) = baseline else {
        return "new".to_owned();
    };

    let mut changes = Vec::new();

    if current.status != baseline.status {
        changes.push(format!(
            "status {:?}->{:?}",
            baseline.status, current.status
        ));
    }

    if current.ref_name != baseline.ref_name {
        changes.push(format!(
            "ref {}->{}",
            baseline.ref_name.as_deref().unwrap_or("-"),
            current.ref_name.as_deref().unwrap_or("-")
        ));
    }

    if current.matrix != baseline.matrix {
        changes.push(format!(
            "matrix {}->{}",
            render_matrix_label(baseline.matrix.as_ref()),
            render_matrix_label(current.matrix.as_ref())
        ));
    }

    if current.outputs != baseline.outputs {
        changes.push(format!(
            "outputs {}->{}",
            render_outputs_label(baseline.outputs.as_ref()),
            render_outputs_label(current.outputs.as_ref())
        ));
    }

    let job_names = baseline
        .jobs
        .keys()
        .chain(current.jobs.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for job_name in job_names {
        let before = baseline.jobs.get(&job_name);
        let after = current.jobs.get(&job_name);
        if before != after {
            changes.push(format!(
                "{} {}->{}",
                job_name,
                before.map(String::as_str).unwrap_or("-"),
                after.map(String::as_str).unwrap_or("-")
            ));
        }
    }

    if changes.is_empty() {
        "same".to_owned()
    } else {
        changes.join("; ")
    }
}

fn render_matrix_value(value: &Value) -> String {
    match value {
        Value::String(inner) => inner.clone(),
        _ => value.to_string(),
    }
}

fn render_matrix_label(matrix: Option<&BTreeMap<String, Value>>) -> String {
    let Some(matrix) = matrix else {
        return "-".to_owned();
    };

    if matrix.is_empty() {
        return "-".to_owned();
    }

    matrix
        .iter()
        .map(|(key, value)| format!("{key}={}", render_matrix_value(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_outputs_label(outputs: Option<&BTreeMap<String, BTreeMap<String, String>>>) -> String {
    let Some(outputs) = outputs else {
        return "-".to_owned();
    };

    if outputs.is_empty() {
        return "-".to_owned();
    }

    outputs
        .iter()
        .flat_map(|(job_name, job_outputs)| {
            job_outputs
                .iter()
                .map(move |(key, value)| format!("{job_name}.{key}={value}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn load_validation_report(path: &Path) -> Result<ValidationReport, AppError> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub fn render_dashboard_markdown(
    current: &ValidationReport,
    baseline: Option<&ValidationReport>,
) -> String {
    let mut markdown = String::new();
    let job_names = collect_job_names(current, baseline);

    let _ = writeln!(markdown, "## Validation Matrix");
    let _ = writeln!(markdown);
    let _ = writeln!(markdown, "- Workflow: `{}`", current.workflow);
    let _ = writeln!(
        markdown,
        "- Declaration: `{}`",
        current.declaration_path.display()
    );
    let _ = writeln!(markdown, "- Current payloads: `{}`", current.actuals.len());
    let _ = writeln!(
        markdown,
        "- Current passed: `{}`",
        count_status(&current.actuals, ValidationStatus::Passed)
    );
    let _ = writeln!(
        markdown,
        "- Current failed: `{}`",
        count_status(&current.actuals, ValidationStatus::Failed)
    );

    if let Some(baseline) = baseline {
        let _ = writeln!(
            markdown,
            "- Baseline payloads: `{}`",
            baseline.actuals.len()
        );
        let _ = writeln!(
            markdown,
            "- Baseline passed: `{}`",
            count_status(&baseline.actuals, ValidationStatus::Passed)
        );
        let _ = writeln!(
            markdown,
            "- Baseline failed: `{}`",
            count_status(&baseline.actuals, ValidationStatus::Failed)
        );
    }

    let _ = writeln!(markdown);
    let _ = write!(markdown, "| Payload | Ref | Matrix | Outputs | Status |");
    for job_name in &job_names {
        let _ = write!(markdown, " {} |", job_name);
    }
    let _ = writeln!(markdown, " Delta |");

    let _ = write!(markdown, "| --- | --- | --- | --- | --- |");
    for _ in &job_names {
        let _ = write!(markdown, " --- |");
    }
    let _ = writeln!(markdown, " --- |");

    let baseline_map = baseline.map(|report| {
        report
            .actuals
            .iter()
            .map(|actual| (actual.actual_path.clone(), actual))
            .collect::<BTreeMap<_, _>>()
    });

    for actual in &current.actuals {
        let _ = write!(
            markdown,
            "| `{}` | `{}` | `{}` | `{}` | `{:?}` |",
            actual.actual_path.display(),
            actual.ref_name.as_deref().unwrap_or("-"),
            render_matrix_label(actual.matrix.as_ref()),
            render_outputs_label(actual.outputs.as_ref()),
            actual.status
        );
        for job_name in &job_names {
            let value = actual.jobs.get(job_name).map(String::as_str).unwrap_or("-");
            let _ = write!(markdown, " `{}` |", value);
        }
        let baseline_actual = baseline_map
            .as_ref()
            .and_then(|entries| entries.get(&actual.actual_path))
            .copied();
        let _ = writeln!(markdown, " {} |", compare_actual(actual, baseline_actual));
    }

    if let Some(baseline) = baseline {
        let current_paths = current
            .actuals
            .iter()
            .map(|actual| actual.actual_path.clone())
            .collect::<BTreeSet<_>>();
        let removed = baseline
            .actuals
            .iter()
            .filter(|actual| !current_paths.contains(&actual.actual_path))
            .collect::<Vec<_>>();

        if !removed.is_empty() {
            let _ = writeln!(markdown);
            let _ = writeln!(markdown, "## Removed Since Baseline");
            let _ = writeln!(markdown);
            for actual in removed {
                let _ = writeln!(markdown, "- `{}`", actual.actual_path.display());
            }
        }
    }

    markdown
}

pub fn write_dashboard_markdown(
    current: &ValidationReport,
    baseline: Option<&ValidationReport>,
    output_path: &Path,
) -> Result<(), AppError> {
    fs::write(output_path, render_dashboard_markdown(current, baseline))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    type MatrixEntries<'a> = &'a [(&'a str, Value)];
    type OutputEntries<'a> = &'a [(&'a str, &'a [(&'a str, &'a str)])];

    fn actual(
        path: &str,
        ref_name: &str,
        status: ValidationStatus,
        matrix: Option<MatrixEntries<'_>>,
        outputs: Option<OutputEntries<'_>>,
        jobs: &[(&str, &str)],
    ) -> ActualValidationReport {
        ActualValidationReport {
            actual_path: PathBuf::from(path),
            workflow: "ci.yml".to_owned(),
            ref_name: Some(ref_name.to_owned()),
            status,
            jobs: jobs
                .iter()
                .map(|(job, result)| (job.to_string(), result.to_string()))
                .collect(),
            matrix: matrix.map(|entries| {
                entries
                    .iter()
                    .map(|(key, value)| (key.to_string(), value.clone()))
                    .collect()
            }),
            outputs: outputs.map(|jobs| {
                jobs.iter()
                    .map(|(job_name, output_entries)| {
                        (
                            job_name.to_string(),
                            output_entries
                                .iter()
                                .map(|(key, value)| (key.to_string(), value.to_string()))
                                .collect(),
                        )
                    })
                    .collect()
            }),
            error: None,
        }
    }

    #[test]
    fn renders_matrix_with_baseline_changes() {
        let current = ValidationReport {
            workflow: "ci.yml".to_owned(),
            declaration_path: PathBuf::from(".github/actionspec/ci/main.cue"),
            actuals: vec![
                actual(
                    "tests/fixtures/ci/ci-main-success.json",
                    "main",
                    ValidationStatus::Passed,
                    Some(&[("app", Value::String("build-ts-service".to_owned()))]),
                    Some(&[("build", &[("contract_build", "build-ts-service")])]),
                    &[("build", "success"), ("pages", "skipped")],
                ),
                actual(
                    "tests/fixtures/ci/ci-build-skipped.json",
                    "main",
                    ValidationStatus::Failed,
                    Some(&[("app", Value::String("build-ts-service".to_owned()))]),
                    Some(&[("build", &[("contract_build", "build-ts-service")])]),
                    &[("build", "skipped"), ("pages", "skipped")],
                ),
            ],
        };
        let baseline = ValidationReport {
            workflow: "ci.yml".to_owned(),
            declaration_path: PathBuf::from(".github/actionspec/ci/main.cue"),
            actuals: vec![
                actual(
                    "tests/fixtures/ci/ci-main-success.json",
                    "main",
                    ValidationStatus::Passed,
                    Some(&[("app", Value::String("build-rust-service".to_owned()))]),
                    Some(&[("build", &[("contract_build", "build-rust-service")])]),
                    &[("build", "success"), ("pages", "success")],
                ),
                actual(
                    "tests/fixtures/ci/ci-removed.json",
                    "main",
                    ValidationStatus::Passed,
                    None,
                    None,
                    &[("build", "success")],
                ),
            ],
        };

        let markdown = render_dashboard_markdown(&current, Some(&baseline));

        assert!(markdown.contains("Validation Matrix"));
        assert!(markdown
            .contains("| Payload | Ref | Matrix | Outputs | Status | build | pages | Delta |"));
        assert!(markdown.contains("app=build-ts-service"));
        assert!(markdown.contains("build.contract_build=build-ts-service"));
        assert!(markdown.contains("matrix app=build-rust-service->app=build-ts-service"));
        assert!(markdown.contains(
            "outputs build.contract_build=build-rust-service->build.contract_build=build-ts-service"
        ));
        assert!(markdown.contains("ci-main-success.json"));
        assert!(markdown.contains("pages success->skipped"));
        assert!(
            markdown.contains(
                "| `tests/fixtures/ci/ci-build-skipped.json` | `main` | `app=build-ts-service` | `build.contract_build=build-ts-service` | `Failed` |"
            )
        );
        assert!(markdown.contains(" new |"));
        assert!(markdown.contains("Removed Since Baseline"));
        assert!(markdown.contains("ci-removed.json"));
    }
}
