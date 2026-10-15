# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# An example of a team's own runner image (docs/runner-image.md): another base, the team's own
# tools, and the contract kept. `runner-image.yml` builds it on every run and checks it with
# `tools/runner-image-contract.mjs`, so the contract is shown to be keepable by an image that is
# not purlis's own.
#
# The build context holds the `purlis` binary, as for purlis's image. A team takes it from
# purlis's image instead, by digest:
#
#   COPY --from=ghcr.io/purlis/purlis-runner@sha256:<digest> /usr/local/bin/purlis /usr/local/bin/purlis
FROM debian:trixie-slim@sha256:a29215f6a35e51e22adffa17f89e9d2ef06214e64a2bad10d765c46aea49f11f

# What the contract asks for (ca-certificates, git, bash, an init), and what this team adds:
# make and openssh-client, as an example.
RUN apt-get update \
 && apt-get install --yes --no-install-recommends ca-certificates git tini make openssh-client \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --uid 20000 --user-group --create-home --home-dir /home/runner --shell /bin/bash runner \
 && chmod 0700 /home/runner

COPY --chmod=0755 purlis /usr/local/bin/purlis

USER 20000:20000
ENV HOME=/home/runner
WORKDIR /home/runner

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/purlis"]
CMD ["serve"]
