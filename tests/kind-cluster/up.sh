#!/usr/bin/env bash
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# **A bare kind cluster, for Pool setup's walk on a cluster in the window** (#4494): a kind
# cluster of the oldest Kubernetes purlis supports (`operator/e2e/kind-node.txt`), with nothing of
# purlis on it, whose nodes pull from this job's own registry, and this build's cluster operator
# image pushed there. Pool setup installs everything else itself, from the window.
#
# Run from the repository's root once the registry runs: `up.sh <operator binary> <registry
# container> <registry port>`. It names, in `$GITHUB_ENV`, the cluster's kubeconfig as
# `KUBECONFIG`, a file of this job's own (no one's own kubeconfig is read or written), and the
# operator's image by digest as `PURLIS_E2E_CLUSTER_OPERATOR_IMAGE`. Nothing is published but the
# cluster's API server, on loopback, as kind does.
set -euo pipefail
operator_binary=${1:?the cluster operator binary of this build}
registry=${2:?the registry container this job pushes to}
port=${3:?the port of the registry on this machine}
node=$(cat operator/e2e/kind-node.txt)
cluster=pool-setup

# Each command that fetches is asked up to four times, as the image step's are.
again() {
  for try in 1 2 3 4; do
    "$@" && return 0
    [ "$try" = 4 ] && break
    echo "::warning::try $try of 4 failed: $*; asking again in $((try * 30)) s"
    sleep $((try * 30))
  done
  return 1
}

KUBECONFIG=$RUNNER_TEMP/pool-setup-kubeconfig
export KUBECONFIG
again docker pull --quiet "$node"
kind create cluster --name "$cluster" --image "$node" --wait 120s
# The nodes pull `localhost:<port>/…` from the job's registry, over kind's network.
for n in $(kind get nodes --name "$cluster"); do
  docker exec "$n" mkdir -p "/etc/containerd/certs.d/localhost:$port"
  printf '[host."http://%s:5000"]\n' "$registry" |
    docker exec -i "$n" cp /dev/stdin "/etc/containerd/certs.d/localhost:$port/hosts.toml"
done
docker network connect kind "$registry"
kubectl version

# The operator's image of this build, as operator.yml's kind job builds it, by digest.
context=$RUNNER_TEMP/cluster-operator
mkdir -p "$context"
cp "$operator_binary" "$context/purlis-cluster-operator"
again docker build --file operator/Dockerfile --tag "localhost:$port/purlis-cluster-operator:scenario" "$context"
docker push --quiet "localhost:$port/purlis-cluster-operator:scenario" >/dev/null
operator=$(docker image inspect --format '{{range .RepoDigests}}{{println .}}{{end}}' \
  "localhost:$port/purlis-cluster-operator:scenario" |
  grep "^localhost:$port/purlis-cluster-operator@sha256:" | head -n 1)
echo "operator image $operator"
{
  echo "KUBECONFIG=$KUBECONFIG"
  echo "PURLIS_E2E_CLUSTER_OPERATOR_IMAGE=$operator"
} >> "$GITHUB_ENV"
