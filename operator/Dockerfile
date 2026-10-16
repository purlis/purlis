# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# purlis's cluster operator image (#2054). Built from a context holding ONE file: the
# `purlis-cluster-operator` binary compiled on Ubuntu 24.04 with the repository's pinned
# toolchain, so nothing else of the repository can reach the image. Built for each architecture
# and checked (`tools/operator-image-check.mjs`) by `.github/workflows/operator-image.yml`.
#
# The base is the runner image's (runner-image/Dockerfile): the C library the binary was linked
# against, pinned by the digest of its multi-architecture index, moved by Dependabot.
FROM ubuntu:24.04@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55

# The operator talks only to the cluster's API server, over TLS with the cluster's own
# certificate authority, so it needs no package from the archive.
COPY --chmod=0755 purlis-cluster-operator /usr/local/bin/purlis-cluster-operator

# A numbered user who is not root, so the cluster can check `runAsNonRoot`.
USER 65532:65532

ENTRYPOINT ["/usr/local/bin/purlis-cluster-operator"]
