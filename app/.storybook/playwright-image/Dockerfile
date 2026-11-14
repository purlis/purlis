# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The image every story picture is drawn in (#2956, `docs/component-stories.md`). Nothing builds
# this file. `stories.yml` runs its jobs in the image it names (`tools/story-image.mjs ref`), and it
# is a Dockerfile so that Dependabot's `playwright` group moves this tag and digest in the one pull
# request that moves the npm packages. `tools/story-image.mjs` refuses a version other than the
# one `app/package-lock.json` locks.
FROM mcr.microsoft.com/playwright:v1.65.0-noble@sha256:6e3fb709361521eb27161606e6da6d2674e5e89fa16516cb37d6fdd91372273d
