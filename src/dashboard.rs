use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::errors::AppError;
use crate::types::{
    ActualValidationReport, ValidationIssue, ValidationIssueKind, ValidationReport,
    ValidationStatus,
};

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

fn count_issue_kinds(actuals: &[ActualValidationReport]) -> BTreeMap<ValidationIssueKind, usize> {
    let mut counts = BTreeMap::new();

    for actual in actuals {
        for issue in &actual.issues {
            *counts.entry(issue.kind).or_insert(0) += 1;
        }
    }

    counts
}

fn compare_actual(
    current: &ActualValidationReport,
    baseline: Option<&ActualValidationReport>,
    output_keys: Option<&BTreeSet<String>>,
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
            render_outputs_label(baseline.outputs.as_ref(), output_keys),
            render_outputs_label(current.outputs.as_ref(), output_keys)
        ));
    }

    if current.issues != baseline.issues {
        changes.push(format!(
            "issues {}->{}",
            render_issue_delta_label(&baseline.issues),
            render_issue_delta_label(&current.issues)
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

fn render_outputs_label(
    outputs: Option<&BTreeMap<String, BTreeMap<String, String>>>,
    output_keys: Option<&BTreeSet<String>>,
) -> String {
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
                .filter(move |(key, _)| {
                    output_keys
                        .map(|allowed| allowed.contains(*key))
                        .unwrap_or(true)
                })
                .map(move |(key, value)| format!("{job_name}.{key}={value}"))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_issue_summary(issues: &[ValidationIssue]) -> String {
    if issues.is_empty() {
        return "-".to_owned();
    }

    issues
        .iter()
        .map(ValidationIssue::summary_label)
        .collect::<Vec<_>>()
        .join("<br>")
}

fn render_issue_delta_label(issues: &[ValidationIssue]) -> String {
    if issues.is_empty() {
        return "0".to_owned();
    }

    issues
        .iter()
        .map(ValidationIssue::delta_label)
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_issue_summary_section(
    markdown: &mut String,
    title: &str,
    counts: &BTreeMap<ValidationIssueKind, usize>,
) {
    // Keep the summary compact because it is repeated in job summaries and PR comments.
    let _ = writeln!(markdown, "{title}");
    let _ = writeln!(markdown);

    if counts.is_empty() {
        let _ = writeln!(markdown, "- none");
        return;
    }

    for (kind, count) in counts {
        let _ = writeln!(markdown, "- {}: `{count}`", kind.label());
    }
}

pub fn load_validation_report(path: &Path) -> Result<ValidationReport, AppError> {
    let contents = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&contents)?)
}

pub fn render_dashboard_markdown(
    current: &ValidationReport,
    baseline: Option<&ValidationReport>,
    output_keys: Option<&BTreeSet<String>>,
) -> String {
    let mut markdown = String::new();
    let job_names = collect_job_names(current, baseline);
    let current_issue_counts = count_issue_kinds(&current.actuals);
    let baseline_issue_counts = baseline.map(|report| count_issue_kinds(&report.actuals));

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
    write_issue_summary_section(&mut markdown, "### Current Issues", &current_issue_counts);

    if let Some(baseline_issue_counts) = baseline_issue_counts.as_ref() {
        let _ = writeln!(markdown);
        write_issue_summary_section(&mut markdown, "### Baseline Issues", baseline_issue_counts);
    }

    let _ = writeln!(markdown);
    let _ = write!(
        markdown,
        "| Payload | Ref | Matrix | Outputs | Issues | Status |"
    );
    for job_name in &job_names {
        let _ = write!(markdown, " {} |", job_name);
    }
    let _ = writeln!(markdown, " Delta |");

    let _ = write!(markdown, "| --- | --- | --- | --- | --- | --- |");
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
            "| `{}` | `{}` | `{}` | `{}` | {} | `{:?}` |",
            actual.actual_path.display(),
            actual.ref_name.as_deref().unwrap_or("-"),
            render_matrix_label(actual.matrix.as_ref()),
            render_outputs_label(actual.outputs.as_ref(), output_keys),
            render_issue_summary(&actual.issues),
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
        let _ = writeln!(
            markdown,
            " {} |",
            compare_actual(actual, baseline_actual, output_keys)
        );
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
    output_keys: Option<&BTreeSet<String>>,
    output_path: &Path,
) -> Result<(), AppError> {
    fs::write(
        output_path,
        render_dashboard_markdown(current, baseline, output_keys),
    )?;
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
        let issues = if status == ValidationStatus::Failed {
            vec![ValidationIssue {
                kind: ValidationIssueKind::ValueConflict,
                path: Some("run.jobs.build.outputs.contract_build".to_owned()),
                message: "conflicting values build-rust-service and build-ts-service".to_owned(),
                expected: Some("build-rust-service".to_owned()),
                actual: Some("build-ts-service".to_owned()),
            }]
        } else {
            Vec::new()
        };

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
            issues,
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

        let markdown = render_dashboard_markdown(
            &current,
            Some(&baseline),
            Some(&BTreeSet::from(["contract_build".to_owned()])),
        );

        assert!(markdown.contains("Validation Matrix"));
        assert!(markdown.contains("### Current Issues"));
        assert!(markdown.contains("- conflict: `1`"));
        assert!(markdown.contains("### Baseline Issues"));
        assert!(markdown.contains("- none"));
        assert!(markdown.contains(
            "| Payload | Ref | Matrix | Outputs | Issues | Status | build | pages | Delta |"
        ));
        assert!(markdown.contains("app=build-ts-service"));
        assert!(markdown.contains("build.contract_build=build-ts-service"));
        assert!(markdown.contains("conflict: run.jobs.build.outputs.contract_build"));
        assert!(!markdown.contains("artifact_name"));
        assert!(markdown.contains("matrix app=build-rust-service->app=build-ts-service"));
        assert!(markdown.contains(
            "outputs build.contract_build=build-rust-service->build.contract_build=build-ts-service"
        ));
        assert!(markdown.contains("ci-main-success.json"));
        assert!(markdown.contains("pages success->skipped"));
        assert!(
            markdown.contains("| `tests/fixtures/ci/ci-build-skipped.json` | `main` | `app=build-ts-service` | `build.contract_build=build-ts-service` | conflict: run.jobs.build.outputs.contract_build | `Failed` |")
        );
        assert!(markdown.contains(" new |"));
        assert!(markdown.contains("Removed Since Baseline"));
        assert!(markdown.contains("ci-removed.json"));
    }

    #[test]
    fn summarizes_multiple_issue_kinds_across_payloads() {
        let report = ValidationReport {
            workflow: "ci.yml".to_owned(),
            declaration_path: PathBuf::from(".github/actionspec/ci/main.cue"),
            actuals: vec![
                ActualValidationReport {
                    actual_path: PathBuf::from("tests/fixtures/ci/ci-main-success.json"),
                    workflow: "ci.yml".to_owned(),
                    ref_name: Some("main".to_owned()),
                    status: ValidationStatus::Failed,
                    jobs: BTreeMap::from([("build".to_owned(), "failure".to_owned())]),
                    matrix: None,
                    outputs: None,
                    issues: vec![
                        ValidationIssue {
                            kind: ValidationIssueKind::ValueConflict,
                            path: Some("run.jobs.build.outputs.contract_build".to_owned()),
                            message: "conflicting values a and b".to_owned(),
                            expected: Some("a".to_owned()),
                            actual: Some("b".to_owned()),
                        },
                        ValidationIssue {
                            kind: ValidationIssueKind::MissingField,
                            path: Some("run.jobs.publish.outputs.image_tag".to_owned()),
                            message: "field is required but not present".to_owned(),
                            expected: None,
                            actual: None,
                        },
                    ],
                    error: Some("cue vet failed".to_owned()),
                },
                ActualValidationReport {
                    actual_path: PathBuf::from("tests/fixtures/ci/ci-main-pages.json"),
                    workflow: "ci.yml".to_owned(),
                    ref_name: Some("main".to_owned()),
                    status: ValidationStatus::Failed,
                    jobs: BTreeMap::from([("pages".to_owned(), "failure".to_owned())]),
                    matrix: None,
                    outputs: None,
                    issues: vec![ValidationIssue {
                        kind: ValidationIssueKind::ValueConflict,
                        path: Some("run.jobs.pages.result".to_owned()),
                        message: "conflicting values success and failure".to_owned(),
                        expected: Some("success".to_owned()),
                        actual: Some("failure".to_owned()),
                    }],
                    error: Some("cue vet failed".to_owned()),
                },
            ],
        };

        let markdown = render_dashboard_markdown(&report, None, None);

        assert!(markdown.contains("### Current Issues"));
        assert!(markdown.contains("- conflict: `2`"));
        assert!(markdown.contains("- missing field: `1`"));
    }
}
