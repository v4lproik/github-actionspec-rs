package actionspec

workflow: "ci.yml"

let detectChanges = run.jobs["detect-changes"]
let runCI = detectChanges.outputs.run_ci
let runPages = detectChanges.outputs.run_pages

run: #Declaration.run & {
  workflow: workflow
  ref:      "main"

  jobs: {
    "detect-changes": {
      result: "success"
      outputs: {
        run_ci:    "true" | "false"
        run_pages: "true" | "false"
      }
    }

    if runCI == "true" {
      lint: {
        result: "success"
      }

      build: {
        result: "success"
      }

      tests: {
        result: "success"
      }

      docker: {
        result: "success"
      }
    }

    if runCI == "false" {
      lint: {
        result: "skipped"
      }

      build: {
        result: "skipped"
      }

      tests: {
        result: "skipped"
      }

      docker: {
        result: "skipped"
      }
    }

    if runPages == "true" {
      pages: {
        result: "success"
      }
    }

    if runPages == "false" {
      pages: {
        result: "skipped"
      }
    }

    if runPages == "true" {
      "detect-changes": {
        outputs: {
          run_ci: "true"
        }
      }
    }
  }
}
