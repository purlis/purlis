# The cluster operator

purlis's **cluster operator** is purlis in your cluster: a controller a team installs in its
own Kubernetes cluster, self-hosted and free (spec #2041). A team admin defines a **pool**
once: where the team's runners come from. A person asks for a runner from it with a
**claim**. The operator turns each claim into a runner, a pod running `purlis serve`, the same
session host as the person's own machine.

It is built on [kubernetes-sigs/agent-sandbox](https://github.com/kubernetes-sigs/agent-sandbox),
pinned to one tested release whose definitions purlis vendors, and it never re-implements what
that project does. Its source is [`operator/`](../operator/).

This is its first slice (#2054): a Pool and a Claim make a running session host, and the
Claim says whether it is ready. The operator's image is not published yet; build it from
[`operator/Dockerfile`](../operator/Dockerfile).

## What it needs

- **Kubernetes 1.36 or later.** The operator's tests run on kind's Kubernetes 1.36.4.
- **Upstream agent-sandbox v1.0.6**, the release purlis is tested against. Its manifest,
  controller and definitions together, is vendored verbatim in
  [`operator/vendor/agent-sandbox/v1.0.6/`](../operator/vendor/agent-sandbox/v1.0.6/).
- **A runner image**, named by digest: purlis's own or a team's that keeps the
  [runner image contract](runner-image.md).

## Install

Three steps, in order. The definitions and their policy are a step of their own, so a
cluster admin can review and apply them apart from the controllers.

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller.
kubectl apply --server-side -f operator/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml

# 2. The policy that admits a Claim only for someone who may use its Pool, then purlis's
#    definitions, Pool and Claim. The policy goes first, so no Claim is admitted before it
#    holds.
kubectl apply --server-side -f operator/deploy/policy.yaml
kubectl apply --server-side -f operator/deploy/crds.yaml

# 3. The operator, in the `purlis-system` namespace, with its image set to your build.
kubectl apply --server-side -f operator/deploy/operator.yaml
kubectl -n purlis-system set image deployment/purlis-cluster-operator \
  operator=<registry>/purlis-cluster-operator@sha256:<digest>
```

## A pool and a claim

```yaml
apiVersion: runners.purlis.dev/v1alpha1
kind: Pool
metadata:
  name: team
spec:
  image: ghcr.io/purlis/purlis-runner@sha256:<digest>
---
apiVersion: runners.purlis.dev/v1alpha1
kind: Claim
metadata:
  name: ada-1
  namespace: ada
spec:
  pool: team
```

- **A Pool** is cluster-wide, defined once for the team. Its `image` is a runner image named
  by digest (`<repository>@sha256:<digest>`); the cluster refuses an image named by tag, since
  a tag can move.
- **A Claim** lives in a namespace and names its `pool`, which never changes: another pool is
  another claim. Its `Ready` condition says whether its runner is running, and if not, why:
  `PoolNotFound` when there is no such pool, `Starting` until upstream reports, and upstream's
  own reason after that. `kubectl get claims` shows it.
- **Deleting a Claim** deletes its runner. Deleting a Pool removes the template and warm pool
  the operator made for it in each namespace, and its Claims then say `PoolNotFound`; each
  runner goes when its Claim is deleted.

`Pool` and `Claim` are `v1alpha1`: no stability promise before upstream agent-sandbox leaves
beta.

**Who may use a Pool is the cluster's own access control.** A Claim is admitted only when the
person making it may `use` the Pool it names: a ValidatingAdmissionPolicy
([`operator/deploy/policy.yaml`](../operator/deploy/policy.yaml)) asks the cluster's
authorizer, as that person, when the Claim is created. Anyone else's Claim is refused with
the pool's name. A cluster admin grants `use` with RBAC, on one pool or on all of them:

```yaml
apiVersion: rbac.authorization.k8s.io/v1
kind: ClusterRole
metadata:
  name: use-pool-team
rules:
  - apiGroups: [runners.purlis.dev]
    resources: [pools]
    resourceNames: [team]
    verbs: [use]
```

Who may create a Claim, and in which namespace, is RBAC on `claims` as usual.

## What it makes

For a Claim in namespace `ns` on Pool `p`, the operator applies, server-side:

| Upstream object | Name | What it holds |
|---|---|---|
| `SandboxTemplate` | `purlis-pool-p` in `ns` | the runner's pod: one container running the pool's image with its own entrypoint (`purlis serve`), as a user who is not root, every capability dropped, no privilege escalation, the runtime's default seccomp profile, no Kubernetes credential mounted and no service addresses in its environment |
| `SandboxWarmPool` | `purlis-pool-p` in `ns` | no runner kept warm: every claim starts one |
| `SandboxClaim` | the Claim's own name in `ns` | the claim on that warm pool |

The template and the warm pool belong to the Pool and the upstream claim to the Claim, so the
cluster's garbage collector removes each with its owner. Upstream's controller makes the pod
from the template and writes the upstream claim's status, which the operator reads.

## What it may do

Its ClusterRole, in [`operator/deploy/operator.yaml`](../operator/deploy/operator.yaml), is all
it holds:

| API group | Resources | Verbs |
|---|---|---|
| `runners.purlis.dev` | `pools` | get, list, watch |
| `runners.purlis.dev` | `claims` | list, watch |
| `runners.purlis.dev` | `claims/status` | patch |
| `extensions.agents.x-k8s.io` | `sandboxclaims` | create, list, patch, watch |
| `extensions.agents.x-k8s.io` | `sandboxtemplates`, `sandboxwarmpools` | create, patch |

It holds no right to `exec`, `attach` or port-forward to a pod, reads no Secret, and touches no
pod at all; its Kubernetes client is built without the websocket support `exec` would need.
`operator/tests/the_install_manifests.rs` holds these rules, and the kind tests ask the cluster
itself. It runs as a user who is not root, with a read-only root file system and every
capability dropped.

Upstream's controller holds its own ClusterRoles, in the vendored manifest: it makes and
removes the runners' pods, services, volume claims and network policies. It holds no Secret
right and no `exec` right either.

**Cluster admins and node owners can read everything in the cluster**, every runner's files
and memory included. Give cluster-admin and node access accordingly.

## Tests

- `cargo test` in `operator/`: what a Claim asks of upstream, that every field the operator
  writes is one the pinned upstream declares, and the install manifests (the definitions are
  generated from the Rust types; regenerate them with
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests`).
- [`operator/e2e/kind.sh`](../operator/e2e/kind.sh) `<runner image> <operator image>`: a kind
  cluster of Kubernetes 1.36 with a registry beside it, so images are named by digest; the
  pinned upstream, purlis's definitions, the policy and the operator; then
  `operator/tests/on_a_kind_cluster.rs`, which checks only what the cluster shows. It removes
  everything it started. `.github/workflows/operator.yml` runs it, with a runner image built
  from the same commit, on every pull request that changes `operator/` or `runner-image/`.

## Moving the upstream pin

Upstream releases weekly. To move to another release: vendor its
`sandbox-with-extensions.yaml` under `operator/vendor/agent-sandbox/<version>/` in place of the
old one, set `VERSION` in `operator/src/upstream.rs` and the path in `operator/e2e/kind.sh`,
and run the tests. `only_what_the_pinned_upstream_defines_is_written` fails when a field the
operator writes is gone or renamed, before a cluster does.
