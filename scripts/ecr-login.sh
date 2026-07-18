#!/usr/bin/env bash
# Log Docker into the ECR registry using AWS_REGION / ECR_REGISTRY from ../.env
set -euo pipefail

DEPLOY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# .env is compose-style (unquoted values may contain spaces) — don't `source` it,
# extract just the two vars we need.
env_get() {
  grep -E "^$1=" "${DEPLOY_DIR}/.env" | tail -n1 | cut -d= -f2- | tr -d '"' | xargs
}

AWS_REGION="$(env_get AWS_REGION)"
ECR_REGISTRY="$(env_get ECR_REGISTRY)"

: "${AWS_REGION:?AWS_REGION missing in .env}"
: "${ECR_REGISTRY:?ECR_REGISTRY missing in .env}"

aws ecr get-login-password --region "${AWS_REGION}" \
  | docker login --username AWS --password-stdin "${ECR_REGISTRY}"
