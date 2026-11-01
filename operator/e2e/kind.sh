#!/bin/sh
# Copyright 2026 Purlis
# SPDX-License-Identifier: Apache-2.0

# The operator's end-to-end tests on a kind cluster (#2054, #2237): a cluster of the oldest
# Kubernetes purlis supports, a registry beside it so images are named by digest as a Pool names
# them, doctor before and after the pinned upstream agent-sandbox (its controller by the digest
# recorded beside the vendored release, checked against the registry), then purlis's definitions,
# policy and operator installed by the Helm chart (charts/purlis-cluster-operator/), as
# docs/cluster-operator.md installs them. The cluster then shows the plain manifests in deploy/
# would change nothing, `tests/on_a_kind_cluster.rs` runs, and then the desktop's side: a chat on
# a cluster runner through the link (crates/purlis-core/tests/, #2056). Everything it started is
# removed when it ends, unless KEEP=1.
#
#   operator/e2e/kind.sh <runner image> <operator image>
#
# Both images are local ones (`docker image ls`): the runner image (runner-image/Dockerfile, or
# any image that keeps its contract) and the operator's (operator/Dockerfile). Needs docker,
# kind, kubectl, helm (v4), cargo, node and openssl on PATH. `.github/workflows/operator.yml` runs it.

set -eu

runner_local=$1
operator_local=$2

here=$(cd "$(dirname "$0")/.." && pwd)
repo=$(cd "$here/.." && pwd)
cluster=${CLUSTER:-purlis-operator-e2e}
registry=$cluster-registry
port=${REGISTRY_PORT:-5001}
# The oldest Kubernetes purlis supports (spec #2041), as kind v0.33.0 publishes it.
node=kindest/node:v1.36.4@sha256:099e049362a1526b2db71494e1947aae99bd16290d7c895f2b7ea312e3cbfaed
registry_image=registry:3@sha256:ddf754342cfc8acc51a56d5d0ab6af06826461864460636d8bd5c546dab2a7b8
# The pinned upstream release, applied as its kustomization there applies it: the vendored
# manifest with the controller named by the release's digest (#2261).
upstream=$here/vendor/agent-sandbox/v1.0.6
# The controller's image, tag and digest, as that record names them.
controller_image=$(sed -n 's/^ *- name: //p' "$upstream/kustomization.yaml")
controller_tag=$(sed -n 's/^ *newTag: //p' "$upstream/kustomization.yaml")
recorded=$(sed -n 's/^ *digest: //p' "$upstream/kustomization.yaml")
if [ -z "$controller_image" ] || [ -z "$controller_tag" ] || [ -z "$recorded" ]; then
  echo "$upstream/kustomization.yaml names no controller image, tag and digest" >&2
  exit 1
fi
context=kind-$cluster
# A kubeconfig of the run's own: the person's own is never read or written.
KUBECONFIG=$(mktemp "${TMPDIR:-/tmp}/purlis-operator-e2e.XXXXXX")
export KUBECONFIG
# Helm's homes of the run's own, likewise.
helm_home=$(mktemp -d "${TMPDIR:-/tmp}/purlis-operator-e2e-helm.XXXXXX")
export HELM_CACHE_HOME="$helm_home/cache" HELM_CONFIG_HOME="$helm_home/config" HELM_DATA_HOME="$helm_home/data"
# The install, pinned to the operator's image by digest, as the operator's image run records
# one for every image it publishes (tools/operator-install.mjs, #2281): the plain manifests
# and the chart, each placeholder replaced by the one tool.
install=$helm_home/install

# Each stage says its name, and in a GitHub Actions run a failure names the stage it stopped in
# as an annotation, beside what it said.
stage=start
step() {
  stage=$1
  echo "--- $1"
}
annotate() {
  if [ -n "${GITHUB_ACTIONS:-}" ]; then
    printf '::error title=%s::%s\n' "$1" "$(printf '%s' "$2" | tail -c 4000 | sed -e ':a' -e 'N' -e '$!ba' -e 's/%/%25/g' -e 's/\n/%0A/g')"
  fi
}

end() {
  status=$?
  if [ "$status" -ne 0 ]; then
    annotate "operator/e2e/kind.sh stopped with $status" "during: $stage"
    echo "--- what the cluster shows" >&2
    kubectl --context "$context" get pools,claims,sandboxclaims,sandboxes,pods -A -o wide >&2 || true
    # What upstream set on each runner's pod that the template did not say.
    kubectl --context "$context" get pods -A -l agents.x-k8s.io/sandbox-name-hash \
      -o jsonpath='{range .items[*]}{.metadata.namespace}/{.metadata.name} dnsPolicy={.spec.dnsPolicy} dnsConfig={.spec.dnsConfig} hostname={.spec.hostname} subdomain={.spec.subdomain}{"\n"}{end}' >&2 || true
    kubectl --context "$context" get pods -A -o jsonpath='{range .items[*]}{.metadata.namespace}/{.metadata.name} labels={.metadata.labels} dnsPolicy={.spec.dnsPolicy} dnsConfig={.spec.dnsConfig}{"\n"}{end}' >&2 || true
    # Each claim's conditions, its word on its runner's key included.
    kubectl --context "$context" get claims -A \
      -o jsonpath='{range .items[*]}{.metadata.namespace}/{.metadata.name}{range .status.conditions[*]}{"\n  "}{.type}={.status} {.reason}: {.message}{end}{"\n"}{end}' >&2 || true
    kubectl --context "$context" -n purlis-system logs deployment/purlis-cluster-operator --tail=100 >&2 || true
    kubectl --context "$context" -n purlis-system logs deployment/purlis-cluster-operator |
      grep -E 'left behind|leave behind|panicked|its claim has gone' | tail -n 30 >&2 || true
    kubectl --context "$context" -n agent-sandbox-system logs deployment/agent-sandbox-controller --tail=100 >&2 || true
    kubectl --context "$context" get events -A --sort-by=.lastTimestamp >&2 || true
  fi
  if [ "${KEEP:-}" != 1 ]; then
    kind delete cluster --name "$cluster" >/dev/null 2>&1 || true
    docker rm -f "$registry" >/dev/null 2>&1 || true
    rm -f "$KUBECONFIG"
    rm -rf "$helm_home"
  else
    echo "kept: the cluster $cluster, its registry, KUBECONFIG=$KUBECONFIG and Helm's homes in $helm_home" >&2
  fi
  exit "$status"
}
trap end EXIT

step "a registry and a kind cluster"
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
node "$repo/tools/operator-install.mjs" "$install" "$operator"

# Doctor (#2236) with the cluster's admin credentials, as a team admin runs it before installing.
doctor() {
  kubectl config use-context "$context" >/dev/null
  cargo run --manifest-path "$here/Cargo.toml" --locked --quiet -- doctor
}

step "doctor on a bare cluster: upstream agent-sandbox is missing"
if found=$(doctor); then
  echo "$found" >&2
  echo "doctor found nothing missing on a cluster without upstream agent-sandbox" >&2
  exit 1
fi
echo "$found"
echo "$found" | grep -q "^missing: upstream agent-sandbox v1.0.6: not installed" || {
  echo "doctor did not name upstream agent-sandbox as missing" >&2
  exit 1
}

step "the recorded digest is what the registry holds under the pinned tag"
docker pull -q "$controller_image:$controller_tag" >/dev/null
published=$(docker image inspect --format '{{range .RepoDigests}}{{println .}}{{end}}' "$controller_image:$controller_tag" |
  grep "^$controller_image@sha256:" | head -n 1)
if [ "${published#*@}" != "$recorded" ]; then
  what="$controller_image:$controller_tag is ${published#*@} in the registry; the record beside the vendored release names $recorded"
  echo "$what" >&2
  annotate "the recorded controller digest is not the pinned release's" "$what"
  exit 1
fi

step "upstream agent-sandbox v1.0.6, its controller by digest"
kubectl --context "$context" apply --server-side -k "$upstream" >/dev/null
kubectl --context "$context" -n agent-sandbox-system rollout status deployment/agent-sandbox-controller --timeout=180s
running=$(kubectl --context "$context" -n agent-sandbox-system get deployment/agent-sandbox-controller \
  -o jsonpath='{.spec.template.spec.containers[0].image}')
case $running in
  "$controller_image@$recorded" | "$controller_image:$controller_tag@$recorded") echo "controller image: $running" ;;
  *)
    echo "upstream's controller runs $running, not $controller_image by $recorded" >&2
    exit 1
    ;;
esac

step "doctor with the pinned upstream: nothing missing"
doctor

step "purlis's definitions, policy and operator, by the chart"
# The release's namespace, made as docs/cluster-operator.md makes it: only pods that meet the
# restricted Pod Security Standard are admitted in it, so the operator's pod has to.
kubectl --context "$context" create namespace purlis-system
kubectl --context "$context" label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
# The key the operator signs runners' link keys with, made before it is installed, as
# docs/cluster-operator.md makes it (#2056).
# Piped, so the key is never written to a file.
openssl genpkey -algorithm ed25519 |
  kubectl --context "$context" -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin >/dev/null
# The chart's own default names the image, as in the install a person downloads.
# It names the one GitHub App key tests/on_a_kind_cluster.rs makes, and no other (#4204).
helm install purlis-cluster-operator "$install/charts/purlis-cluster-operator" --kube-context "$context" --namespace purlis-system \
  --set 'forge.keySecretNames[0]=e2e-forge-app-key' --wait --timeout 180s
kubectl --context "$context" wait --for=condition=Established --timeout=60s crd/pools.runners.purlis.dev crd/claims.runners.purlis.dev
kubectl --context "$context" -n purlis-system rollout status deployment/purlis-cluster-operator --timeout=120s

step "the plain manifests are what the chart installed"
# `kubectl diff` exits 1 when applying them would change anything the chart made. Helm owns
# those fields, and the policy's match rules are one atomic field the server fills defaults
# into, so the dry run takes the fields over (`--force-conflicts`) to compare values only.
grep -qxF "          image: $operator" "$install/operator.yaml"
if ! diff=$(kubectl --context "$context" diff --server-side --force-conflicts \
  -f "$install/policy.yaml" -f "$install/crds.yaml" -f "$install/operator.yaml" 2>&1); then
  echo "$diff" >&2
  annotate "the plain manifests are not what the chart installed" "$diff"
  exit 1
fi

step "the operator's image trusts the public certificate authorities"
docker run --rm --entrypoint /bin/sh "$operator_local" -c 'test -s /etc/ssl/certs/ca-certificates.crt'

step "the operator asks a stand-in for GitHub"
# The cluster's gateway on kind's network, where tests/minting_on_a_kind_cluster.rs listens: no App
# of ours is on real GitHub for a pull request's run. Set after the diff above, since no install
# sets this.
fake_github_port=${FAKE_GITHUB_PORT:-18080}
gateway=$(docker exec "$cluster-control-plane" ip -4 route show default | sed -n 's/^default via \([0-9.]*\).*/\1/p' | head -n 1)
if [ -z "$gateway" ]; then
  echo "kind's node has no IPv4 default route" >&2
  exit 1
fi
kubectl --context "$context" -n purlis-system set env deployment/purlis-cluster-operator \
  "PURLIS_FORGE_API_URL=http://$gateway:$fake_github_port" \
  PURLIS_OPERATOR_STUCK_AFTER=90 PURLIS_OPERATOR_ORPHANS_EVERY=30 PURLIS_OPERATOR_RETENTION_FLOOR=1 >/dev/null
kubectl --context "$context" -n purlis-system rollout status deployment/purlis-cluster-operator --timeout=120s

step "the policy settles"
# The operator acts only on Claims made more than 30 seconds after the policy's binding
# (SETTLE in src/admission.rs); the tests make theirs at once.
sleep 32

step "the tests"
kubectl config use-context "$context" >/dev/null
PURLIS_RUNNER_IMAGE=$runner cargo test --manifest-path "$here/Cargo.toml" --locked \
  --test on_a_kind_cluster -- --ignored --test-threads=1 --show-output

step "a vouched runner gets a token, and everyone else a refusal"
FAKE_GITHUB_PORT=$fake_github_port PURLIS_RUNNER_IMAGE=$runner cargo test --manifest-path "$here/Cargo.toml" --locked \
  --test minting_on_a_kind_cluster -- --ignored --test-threads=1

step "a chat runs on a cluster runner, through the link"
# The desktop's side (#2056), from the root workspace: a Claim made as this kubeconfig's user,
# the operator's statement checked against the key the Pool names, a pairing over kubectl exec
# that must agree with it, then a link through kubectl exec and a chat on it.
cargo build --manifest-path "$repo/Cargo.toml" --locked -p purlis-cli
CLUSTER_POOL_TEST_PURLIS=$repo/target/debug/purlis \
CLUSTER_POOL_TEST_IMAGE=$runner CLUSTER_POOL_TEST_CONTEXT=$context \
  "$repo/tools/tests-ran.sh" -- cargo test --manifest-path "$repo/Cargo.toml" --locked -p purlis-core \
  --test it -- a_chat_runs_on_a_cluster_runner_through_kubectl_exec:: --ignored --test-threads=1 --nocapture

step "a whole chat on a Pool that mints, with its desktop away"
# The stand-in for GitHub is this test's own now: the one above has ended.
CLUSTER_POOL_TEST_IMAGE=$runner CLUSTER_POOL_TEST_CONTEXT=$context FAKE_GITHUB_PORT=$fake_github_port \
  "$repo/tools/tests-ran.sh" -- cargo test --manifest-path "$repo/Cargo.toml" --locked -p purlis-core \
  --test it -- a_chat_mints_on_a_cluster_runner_with_its_desktop_away:: --ignored --test-threads=1

step "uninstalled, the chart leaves the definitions and the policy"
helm uninstall purlis-cluster-operator --kube-context "$context" --namespace purlis-system --wait
kubectl --context "$context" get crd/pools.runners.purlis.dev crd/claims.runners.purlis.dev \
  validatingadmissionpolicy/purlis-claims-use-their-pool \
  validatingadmissionpolicybinding/purlis-claims-use-their-pool \
  validatingadmissionpolicy/purlis-forge \
  validatingadmissionpolicybinding/purlis-forge \
  validatingadmissionpolicy/purlis-operator-writes-only-a-runners-shape \
  validatingadmissionpolicybinding/purlis-operator-writes-only-a-runners-shape \
  validatingadmissionpolicy/purlis-operator-changes-only-a-sandboxs-mode \
  validatingadmissionpolicybinding/purlis-operator-changes-only-a-sandboxs-mode >/dev/null
