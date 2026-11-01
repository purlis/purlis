# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The image every story picture is drawn in (#2956, `docs/component-stories.md`). Nothing builds
# this file. `stories.yml` runs its jobs in the image it names (`tools/story-image.mjs ref`), and it
# is a Dockerfile so that Dependabot's `playwright` group moves this tag and digest in the one pull
# request that moves the npm packages. `tools/story-image.mjs` refuses a version other than the
# one `app/package-lock.json` locks.
FROM mcr.microsoft.com/playwright:v1.64.1-noble@sha256:47b510dd9230bf0b9bfc9b20102241e577ea7a99334ab4c3ca19f19770e9b8ea
