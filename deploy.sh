#!/bin/bash
set -euo pipefail

# === Configuration ===
ECR_IMAGE="337392631707.dkr.ecr.us-east-1.amazonaws.com/psudo:latest"
REGION="us-east-1"

echo "📦 Installing dependencies and building frontend..."
pnpm install --frozen-lockfile
pnpm run wasm-build
pnpm run build

echo "🔨 Building Docker image..."
docker build --platform linux/amd64 -t psudo .

echo "🔐 Logging in to ECR..."
aws ecr get-login-password --region "$REGION" | docker login --username AWS --password-stdin "${ECR_IMAGE%:*}"

echo "📤 Pushing image to ECR..."
docker tag psudo:latest "$ECR_IMAGE"
docker push "$ECR_IMAGE"
