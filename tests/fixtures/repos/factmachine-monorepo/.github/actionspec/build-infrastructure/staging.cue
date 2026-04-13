package actionspec

workflow: "build-infrastructure.yml"

run: #Declaration.run & {
  workflow: workflow
  ref:      "staging"

  inputs: {
    current_env: "staging"
    target_env:  "staging"
  }

  let bootstrapJob = jobs.bootstrap
  let buildContractJob = jobs["build-contract"]
  let buildSummaryJob = jobs["summarize-build-ts-services"]
  let testServicesJob = jobs["test-ts-services"]
  let testPackagesJob = jobs["test-ts-packages"]
  let coverageJob = jobs["report-coverage"]
  let migrateDBJob = jobs["migrate-db"]
  let promoteServicesJob = jobs["promote-ts-services"]

  jobs: {
    bootstrap: {
      result: "success"
      outputs: {
        current_env:           "staging"
        target_env:            "staging"
        force_trigger:         "true" | "false"
        backend:               "true" | "false"
        frontend:              "true" | "false"
        admin:                 "true" | "false"
        "admin-backend":       "true" | "false"
        worker:                "true" | "false"
        indexer:               "true" | "false"
        monitor:               "true" | "false"
        listener:              "true" | "false"
        contract:              "true" | "false"
        packages:              "true" | "false"
        packages_non_contract: "true" | "false"
      }
    }

    "lint-gate": {
      result: "success"
    }

    "build-ts-services": {
      result: "success" | "skipped"
    }

    "build-contract": {
      result: "success" | "skipped"
      if result == "success" {
        outputs: {
          validator_runtime_tag: string & != ""
        }
      }
    }

    // The build summary is the downstream source of truth for runtime tags.
    // It must carry a validator runtime tag even when `build-contract` is skipped.
    "summarize-build-ts-services": {
      result: "success"
      outputs: {
        "backend-runtime-tag":       string & != ""
        "frontend-runtime-tag":      string & != ""
        "admin-runtime-tag":         string & != ""
        "admin-backend-runtime-tag": string & != ""
        "worker-runtime-tag":        string & != ""
        "indexer-runtime-tag":       string & != ""
        "monitor-runtime-tag":       string & != ""
        "listener-runtime-tag":      string & != ""
        "validator-runtime-tag":     string & != ""
      }
    }

    "test-ts-services": {
      result: "success" | "failure"
    }

    "test-ts-packages": {
      result: "success" | "failure" | "skipped"
    }

    "report-coverage": {
      result: "success"
      outputs: {
        tests_passed: "true" | "false"
      }
    }

    "migrate-db": {
      result: "success" | "skipped"
    }

    "promote-ts-services": {
      result: "success" | "skipped"
    }

    "summarize-promote-ts-services": {
      result: "success" | "skipped"
    }

    "deploy-backend": {
      result: "skipped"
    }

    "deploy-frontend": {
      result: "skipped"
    }

    "deploy-admin": {
      result: "skipped"
    }

    "deploy-worker": {
      result: "skipped"
    }

    "deploy-indexer": {
      result: "skipped"
    }

    "deploy-monitor": {
      result: "skipped"
    }

    "deploy-admin-backend": {
      result: "skipped"
    }
  }

  _bootstrap_current_env_matches_inputs: bootstrapJob.outputs.current_env & inputs.current_env
  _bootstrap_target_env_matches_inputs:  bootstrapJob.outputs.target_env & inputs.target_env

  if buildContractJob.result == "success" {
    _build_contract_validator_tag_present: buildContractJob.outputs.validator_runtime_tag & string & != ""
  }

  _build_summary_validator_tag_present: buildSummaryJob.outputs["validator-runtime-tag"] & string & != ""

  // Promotion is gated by the safeguard output, not by raw upstream job results.
  // When the safeguard says tests passed, the required test jobs must agree.
  if coverageJob.outputs.tests_passed == "true" {
    _test_services_gate:    testServicesJob.result & "success"
    _test_packages_gate:    testPackagesJob.result & ("success" | "skipped")
    _migrate_db_gate:       migrateDBJob.result & ("success" | "skipped")
    _promote_services_gate: promoteServicesJob.result & ("success" | "skipped")
  }

  if coverageJob.outputs.tests_passed == "false" {
    _migrate_db_skips_on_failed_tests:      migrateDBJob.result & "skipped"
    _promote_services_skip_on_failed_tests: promoteServicesJob.result & "skipped"
  }
}
