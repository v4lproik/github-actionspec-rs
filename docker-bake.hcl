variable "IMAGE_TAG" {
  default = "github-actionspec-rs-dev:local"
}

variable "RUNTIME_IMAGE_TAG" {
  default = "github-actionspec-rs:local"
}

variable "CUE_VERSION" {
  default = "v0.15.0"
}

target "dev" {
  # Keep a single dev target for every local and CI verification flow so the image
  # contract lives here instead of being duplicated across just recipes and workflows.
  context = "."
  dockerfile = "Dockerfile"
  target = "dev"
  args = {
    CUE_VERSION = CUE_VERSION
  }
  tags = [IMAGE_TAG]
}

target "runtime" {
  context = "."
  dockerfile = "Dockerfile"
  target = "runtime"
  args = {
    CUE_VERSION = CUE_VERSION
  }
  tags = [RUNTIME_IMAGE_TAG]
}

group "default" {
  targets = ["dev", "runtime"]
}
