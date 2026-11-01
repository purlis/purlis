# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The image every story picture is drawn in (#2956, `docs/component-stories.md`). Nothing builds
# this file. `stories.yml` runs its jobs in the image it names (`tools/story-image.mjs ref`), and it
# is a Dockerfile so that Dependabot's `playwright` group moves this tag and digest in the one pull
# request that moves the npm packages. `tools/story-image.mjs` refuses a version other than the
# one `app/package-lock.json` locks.
FROM mcr.microsoft.com/playwright:v1.64.1-noble@sha256:7852dc5a7d72f5fe458a597d690f6c42fbc8e9e26ddfa95f8e6ef552558d9bbf
