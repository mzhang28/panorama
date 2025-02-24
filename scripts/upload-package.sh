#!/usr/bin/env bash

set -euo pipefail

TAG="git-$(git rev-parse --short HEAD)-dirty-$(git diff | sha256sum | cut -d' ' -f1)"
REMOTE_IMAGE_NAME="git.mzhang.io/michael/panorama:$TAG"
echo "$REMOTE_IMAGE_NAME"
# docker image tag "$IMAGE_NAME" "$REMOTE_IMAGE_NAME"
podman build -t "$REMOTE_IMAGE_NAME" .

# sed -i -E "s~(.*image: ).*blog-docker-builder:?.*~\1$REMOTE_IMAGE_NAME~" .woodpecker/deploy.yml
# echo "Created $REMOTE_IMAGE_NAME"

podman push -q "$REMOTE_IMAGE_NAME"
