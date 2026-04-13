package actionspec

workflow: "ci.yml"

run: #Declaration.run & {
  workflow: workflow
  ref:      "main"

  jobs: {
    "detect-changes": {
      result: "success"
    }

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

    // Pages only runs for docs changes on pushes to main.
    pages: {
      result: "success" | "skipped"
    }
  }
}
