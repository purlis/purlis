# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The image every story picture is drawn in (#2956, `docs/component-stories.md`). Nothing builds
# this file. `stories.yml` runs its jobs in the image it names (`tools/story-image.mjs ref`), and it
# is a Dockerfile so that Dependabot's `playwright` group moves this tag and digest in the one pull
# request that moves the npm packages. `tools/story-image.mjs` refuses a version other than the
# one `app/package-lock.json` locks.
FROM mcr.microsoft.com/playwright:v1.65.1-noble@sha256:1507cbfc550a63a6b7396791ca4f00df17beb56342d2447b3f106c306f525579
