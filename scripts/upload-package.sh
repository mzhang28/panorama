#!/usr/bin/env bash

set -euo pipefail

if command -v podman &> /dev/null; then
    CONTAINER_TOOL="podman"
elif command -v docker &> /dev/null; then
    CONTAINER_TOOL="docker"
else
    echo "Neither podman nor docker is installed."
    exit 1
fi

TAG="git-$(git rev-parse --short HEAD)-dirty-$(git diff | sha256sum | cut -d' ' -f1)"
REMOTE_IMAGE_NAME="git.mzhang.io/michael/panorama:$TAG"
echo "$REMOTE_IMAGE_NAME"
# docker image tag "$IMAGE_NAME" "$REMOTE_IMAGE_NAME"
$CONTAINER_TOOL build -t "$REMOTE_IMAGE_NAME" .

# sed -i -E "s~(.*image: ).*blog-docker-builder:?.*~\1$REMOTE_IMAGE_NAME~" .woodpecker/deploy.yml
# echo "Created $REMOTE_IMAGE_NAME"

$CONTAINER_TOOL push -q "$REMOTE_IMAGE_NAME"
