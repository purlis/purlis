# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# purlis's runner image (#2043): `purlis serve`, the session host, on a machine with no window.
# What any runner image has to hold is docs/runner-image.md; this is purlis's own, and
# `tools/runner-image-contract.mjs` checks it against that contract before it is pushed.
#
# Built by `.github/workflows/runner-image.yml`, once per architecture on a runner of that
# architecture, from a build context holding the `purlis` binary that job compiled with the
# repository's pinned toolchain, and purlis's plugin (`app/src-tauri/plugin`) as the commit has
# it. Nothing else of the repository is in the context, so nothing else can reach the image.
#
# The base is Ubuntu 26.04 LTS. The binary is compiled on Ubuntu 24.04 and needs glibc 2.39 or
# later; a newer glibc runs what an older one linked. The base is pinned by the digest of its
# multi-architecture index; Dependabot proposes the next one. The packages are the archive's
# current ones at build time, so every build of `main` carries the security fixes of that day.

# `gh` and `glab` (#4354): each vendor's own release package, pinned by version and checked by
# SHA-256 as the file is fetched, for the architecture being built. The sums were read from each
# release's published checksums file. Neither program's package has an install script, and each
# depends only on git. Ubuntu's archive is not used: a package of it stays near the version the
# release froze on, and these two move faster. Moving to a new version is its URLs and sums in
# both stages, by hand. No credential comes with them: on a
# runner, without a persona's declared tool, each is simply not signed in (ADR 0077).
FROM scratch AS forge-programs-amd64
ADD --checksum=sha256:e3bafef96328d1a67de3a968bdfe1cc2ffd58cf6aba8b5e201356897ed06bb3d \
    https://github.com/cli/cli/releases/download/v2.103.3/gh_2.103.3_linux_amd64.deb /gh.deb
ADD --checksum=sha256:41583b9801086dd347fc6480b6c4597178c2e39e99e491e961a7179cd87f0407 \
    https://gitlab.com/gitlab-org/cli/-/releases/v1.126.0/downloads/glab_1.126.0_linux_amd64.deb /glab.deb

FROM scratch AS forge-programs-arm64
ADD --checksum=sha256:cac3ff84b60b68e64cca69f31f303210c26320361cc3286b773655d4226b7b06 \
    https://github.com/cli/cli/releases/download/v2.103.3/gh_2.103.3_linux_arm64.deb /gh.deb
ADD --checksum=sha256:2b9989364afacf34a9a75328a241b596df4318cfc8c11f437b8d65ca24d3fe7e \
    https://gitlab.com/gitlab-org/cli/-/releases/v1.126.0/downloads/glab_1.126.0_linux_arm64.deb /glab.deb

ARG TARGETARCH
FROM forge-programs-${TARGETARCH} AS forge-programs

FROM ubuntu:26.04@sha256:c098726dd9b9733f3e885f8de0ef375206a311b0a317eadc40d347b1115fb944

# git: a chat's clone and branches. ca-certificates: every TLS connection a chat makes.
# tini: PID 1, which reaps what a chat's programs leave behind when the host is not there to.
# The rest is what the pinned browser engine needs to run (#3298): the libraries it links (its
# own `deb.deps`, measured on this base 2026-10-28) and one font family, so a page's text is
# drawn. The engine itself is not in the image: the runner's host downloads it, checked, on its
# first browser, as on any machine.
# `gh` and `glab` are installed from the packages checked above, listed by dpkg, so the image's
# SBOM names them; the packages themselves are mounted, and stay out of the image.
# Ubuntu's own `ubuntu` user (1000) is removed, so the image has one user who is not root.
RUN --mount=type=bind,from=forge-programs,target=/forge-programs apt-get update \
 && apt-get install --yes --no-install-recommends ca-certificates git tini \
      fonts-liberation libasound2t64 libatk-bridge2.0-0t64 libatk1.0-0t64 libatspi2.0-0t64 \
      libcairo2 libcups2t64 libdbus-1-3 libdrm2 libexpat1 libgbm1 libglib2.0-0t64 libnspr4 \
      libnss3 libpango-1.0-0 libudev1 libx11-6 libxcb1 libxcomposite1 libxdamage1 libxext6 \
      libxfixes3 libxkbcommon0 libxrandr2 \
 && dpkg --install /forge-programs/gh.deb /forge-programs/glab.deb \
 && rm -rf /var/lib/apt/lists/* \
 && userdel --remove ubuntu \
 && groupadd --gid 10001 purlis \
 && useradd --uid 10001 --gid 10001 --create-home --home-dir /home/purlis --shell /bin/bash purlis \
 && chmod 0700 /home/purlis

COPY --chmod=0755 purlis /usr/local/bin/purlis
# purlis's plugin, beside the binary, where `purlis serve` finds it: a sandboxed Claude Code
# chat is armed with it (#2447). Read-only to the image's user.
COPY plugin /usr/local/lib/purlis/plugin

USER 10001:10001
ENV HOME=/home/purlis
WORKDIR /home/purlis

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/purlis"]
CMD ["serve"]
