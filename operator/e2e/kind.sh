#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The operator's end-to-end tests on a kind cluster (#2054): a cluster of the oldest Kubernetes
# purlis supports, a registry beside it so images are named by digest as a Pool names them, the
# pinned upstream agent-sandbox, purlis's definitions and the operator, then
# `tests/on_a_kind_cluster.rs`. Everything it started is removed when it ends, unless KEEP=1.
#
#   operator/e2e/kind.sh <runner image> <operator image>
#
# Both images are local ones (`docker image ls`): the runner image (runner-image/Dockerfile, or
# any image that keeps its contract) and the operator's (operator/Dockerfile). Needs docker,
# kind, kubectl and cargo on PATH. `.github/workflows/operator.yml` runs it.

set -eu

runner_local=$1
operator_local=$2

here=$(cd "$(dirname "$0")/.." && pwd)
cluster=${CLUSTER:-purlis-operator-e2e}
registry=$cluster-registry
port=${REGISTRY_PORT:-5001}
# The oldest Kubernetes purlis supports (spec #2041), as kind v0.33.0 publishes it.
node=kindest/node:v1.36.4@sha256:099e049362a1526b2db71494e1947aae99bd16290d7c895f2b7ea312e3cbfaed
registry_image=registry:3@sha256:ddf754342cfc8acc51a56d5d0ab6af06826461864460636d8bd5c546dab2a7b8
upstream=$here/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml
context=kind-$cluster
# A kubeconfig of the run's own: the person's own is never read or written.
KUBECONFIG=$(mktemp "${TMPDIR:-/tmp}/purlis-operator-e2e.XXXXXX")
export KUBECONFIG

end() {
  status=$?
  if [ "$status" -ne 0 ]; then
    echo "--- what the cluster shows" >&2
    kubectl --context "$context" get pools,claims,sandboxclaims,sandboxes,pods -A -o wide >&2 || true
    kubectl --context "$context" -n purlis-system logs deployment/purlis-cluster-operator --tail=100 >&2 || true
    kubectl --context "$context" -n agent-sandbox-system logs deployment/agent-sandbox-controller --tail=100 >&2 || true
    kubectl --context "$context" get events -A --sort-by=.lastTimestamp >&2 || true
  fi
  if [ "${KEEP:-}" != 1 ]; then
    kind delete cluster --name "$cluster" >/dev/null 2>&1 || true
    docker rm -f "$registry" >/dev/null 2>&1 || true
    rm -f "$KUBECONFIG"
  else
    echo "kept: the cluster $cluster, its registry, and KUBECONFIG=$KUBECONFIG" >&2
  fi
  exit "$status"
}
trap end EXIT

echo "--- a registry and a kind cluster"
docker run -d --restart=no -p "127.0.0.1:$port:5000" --name "$registry" "$registry_image" >/dev/null
# API_PORT fixes the API server's port on this machine, where kind would otherwise pick one.
printf 'kind: Cluster\napiVersion: kind.x-k8s.io/v1alpha4\nnetworking:\n  apiServerPort: %s\n' "${API_PORT:-0}" |
  kind create cluster --name "$cluster" --image "$node" --wait 120s --config -
for n in $(kind get nodes --name "$cluster"); do
  docker exec "$n" mkdir -p "/etc/containerd/certs.d/localhost:$port"
  printf '[host."http://%s:5000"]\n' "$registry" |
    docker exec -i "$n" cp /dev/stdin "/etc/containerd/certs.d/localhost:$port/hosts.toml"
done
docker network connect kind "$registry"

# Pushed, then named by the digest the registry holds.
by_digest() {
  docker tag "$1" "localhost:$port/$2:e2e"
  docker push -q "localhost:$port/$2:e2e" >/dev/null
  docker image inspect --format '{{range .RepoDigests}}{{println .}}{{end}}' "localhost:$port/$2:e2e" |
    grep "^localhost:$port/$2@sha256:" | head -n 1
}
runner=$(by_digest "$runner_local" purlis-runner)
operator=$(by_digest "$operator_local" purlis-cluster-operator)
echo "runner image: $runner"
echo "operator image: $operator"

echo "--- upstream agent-sandbox v1.0.6"
kubectl --context "$context" apply --server-side -f "$upstream" >/dev/null
kubectl --context "$context" -n agent-sandbox-system rollout status deployment/agent-sandbox-controller --timeout=180s

echo "--- purlis's definitions, then the operator"
kubectl --context "$context" apply --server-side -f "$here/deploy/crds.yaml"
kubectl --context "$context" wait --for=condition=Established --timeout=60s crd/pools.runners.purlis.dev crd/claims.runners.purlis.dev
sed "s|image: ghcr.io/purlis/purlis-cluster-operator:main|image: $operator|" "$here/deploy/operator.yaml" |
  kubectl --context "$context" apply --server-side -f -
kubectl --context "$context" -n purlis-system rollout status deployment/purlis-cluster-operator --timeout=120s

echo "--- the tests"
kubectl config use-context "$context" >/dev/null
PURLIS_RUNNER_IMAGE=$runner cargo test --manifest-path "$here/Cargo.toml" --locked \
  --test on_a_kind_cluster -- --ignored --test-threads=1
