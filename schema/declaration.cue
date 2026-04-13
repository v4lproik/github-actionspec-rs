package actionspec

#Declaration: {
  workflow: string & != ""
  run: #WorkflowRun & {
    workflow: workflow
  }
}
