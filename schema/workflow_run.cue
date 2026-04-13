package actionspec

#Result: "success" | "failure" | "skipped" | "cancelled" | "timed_out" | "neutral" | "action_required" | "startup_failure" | "stale"

#Step: {
  conclusion?: #Result | string
  outputs?: [string]: string
}

#Job: {
  result: #Result | string
  outputs?: [string]: string
  matrix?: [string]: string | number | bool | null
  steps?: [string]: #Step
}

#WorkflowRun: {
  workflow: string & != ""
  ref?: string & != ""
  inputs?: [string]: string | null
  jobs: [string]: #Job
}
