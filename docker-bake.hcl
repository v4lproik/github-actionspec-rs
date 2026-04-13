variable "IMAGE_TAG" {
  default = "github-actionspec-rs-dev:local"
}

variable "RUNTIME_IMAGE_TAG" {
  default = "github-actionspec-rs:local"
}

target "dev" {
  # Keep a single dev target for every local and CI verification flow so the image
  # contract lives here instead of being duplicated across just recipes and workflows.
  context = "."
  dockerfile = "Dockerfile"
  target = "dev"
  tags = [IMAGE_TAG]
}

target "runtime" {
  context = "."
  dockerfile = "Dockerfile"
  target = "runtime"
  tags = [RUNTIME_IMAGE_TAG]
}

group "default" {
  targets = ["dev", "runtime"]
}
