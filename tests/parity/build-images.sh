#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **The runner image of this build, and the parity suite's around it, by digest** (#2045): what
# the parity jobs against a Docker pool, on this machine (`parity`) and on another reached over
# SSH (`remote parity`, #6266), each build first. Run from the repository's root in CI, after
# `purlis` and the fake harness are built; it writes the images' digests to `$GITHUB_ENV`.
set -euo pipefail
docker run --detach --publish 127.0.0.1:5000:5000 --name registry registry:3@sha256:01cd312e67026f06f5d89c3e133a4a9adc7cb9eeb9b1194e0b2954dfd770efd2
context="$RUNNER_TEMP/runner"
mkdir -p "$context"
cp target/debug/purlis "$context/purlis"
cp -R app/src-tauri/plugin "$context/plugin"
# gh's and glab's packages, fetched and checked against the Dockerfile's sums before the
# build, which then runs none of its ADDs (runner-image/fetch-forge-programs.sh).
arch=$(dpkg --print-architecture)
runner-image/fetch-forge-programs.sh "$arch" "$RUNNER_TEMP/forge-programs"
docker build --file runner-image/Dockerfile --build-context "forge-programs-$arch=$RUNNER_TEMP/forge-programs" \
  --tag purlis-runner:parity "$context"
# The runner image as it ships, with no harness, and one an update moves it to, for the
# test of a person's whole way through a pool (`behaviour::pool_journey`).
docker tag purlis-runner:parity localhost:5000/purlis-runner:shipped
docker push localhost:5000/purlis-runner:shipped
echo "DOCKER_POOL_JOURNEY_IMAGE=$(docker image inspect --format '{{index .RepoDigests 0}}' localhost:5000/purlis-runner:shipped | sed 's/.*@//')" >> "$GITHUB_ENV"
printf 'FROM purlis-runner:parity\nLABEL dev.purlis.parity.next=true\n' \
  | docker build --tag localhost:5000/purlis-runner:next -
docker push localhost:5000/purlis-runner:next
echo "DOCKER_POOL_JOURNEY_NEXT_IMAGE=$(docker image inspect --format '{{index .RepoDigests 0}}' localhost:5000/purlis-runner:next | sed 's/.*@//')" >> "$GITHUB_ENV"
parity="$RUNNER_TEMP/parity"
mkdir -p "$parity"
cp target/debug/purlis target/debug/fake-harness "$parity/"
# The Claude Code purlis supports, this runner's build of it from npm with no install
# scripts run, for the behaviour that runs it inside the wrap (#4083). No model is
# called, and no credential is used.
version=$(sed 's/ .*//' tests/fixtures/harness-versions/claude.txt)
package="@anthropic-ai/claude-code-linux-x64"
# From the committed lockfile, so npm checks the tarball against its integrity; npm
# installs only the build for this runner's os and cpu. The lockfile names the version
# claude.txt does, or the step stops here.
mkdir -p "$RUNNER_TEMP/claude-code"
cp tests/fixtures/harness-versions/claude-code/package.json tests/fixtures/harness-versions/claude-code/package-lock.json "$RUNNER_TEMP/claude-code/"
locked=$(jq -r --arg p "node_modules/$package" '.packages[$p].version' "$RUNNER_TEMP/claude-code/package-lock.json")
if [ "$locked" != "$version" ]; then
  echo "::error file=tests/fixtures/harness-versions/claude-code/package-lock.json::the lockfile holds $package $locked and claude.txt says $version: set the version in its package.json and run npm install --package-lock-only there"
  exit 1
fi
(cd "$RUNNER_TEMP/claude-code" && npm ci --ignore-scripts --no-audit --no-fund)
cp "$RUNNER_TEMP/claude-code/node_modules/$package/claude" "$parity/claude-code"
docker build --file tests/parity/runner.Dockerfile \
  --build-arg RUNNER=purlis-runner:parity --build-arg "AT=$PWD/target/debug" \
  --build-arg "ENGINE_NOW=$(jq -r .issued crates/browser-engine/engine-pin.json)" \
  --tag localhost:5000/purlis-runner-parity:candidate "$parity"
docker push localhost:5000/purlis-runner-parity:candidate
image=$(docker image inspect --format '{{index .RepoDigests 0}}' localhost:5000/purlis-runner-parity:candidate)
echo "image $image"
echo "DOCKER_POOL_TEST_IMAGE=$image" >> "$GITHUB_ENV"
