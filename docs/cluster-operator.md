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
Claim says whether it is ready.

## What it needs

- **Kubernetes 1.36 or later.** The operator's tests run on kind's Kubernetes 1.36.4.
- **Upstream agent-sandbox v1.0.6**, the release purlis is tested against. Its manifest,
  controller and definitions together, is vendored verbatim in
  [`operator/vendor/agent-sandbox/v1.0.6/`](../operator/vendor/agent-sandbox/v1.0.6/).
- **A runner image**, named by digest: purlis's own or a team's that keeps the
  [runner image contract](runner-image.md).

## Its image

- **Where:** `ghcr.io/purlis/purlis-cluster-operator`, a private package of the `purlis`
  organisation on GitHub, kept private exactly as the [runner image](runner-image.md) is: the
  workflow checks through GitHub's packages API that the package is private before each push,
  and again, with the registry refusing both images to anyone who has not signed in, before
  any tag or signature. If either check cannot get a clear answer, the run stops. It stays
  private until the maintainer rules to publish it, so pulling it needs a GitHub sign-in with
  read access to the package: give the cluster an image pull secret for it.
- **When:** `.github/workflows/operator-image.yml` builds it for every change to the operator
  on `main`, once the operator's own checks and kind tests (`operator.yml`) passed on that
  commit. A pull request that changes the operator builds and checks it the same way, and
  pushes nothing.
- **What:** `linux/amd64` and `linux/arm64` under one index, each built on a GitHub runner of
  that architecture with the pinned toolchain, `cargo auditable` and the operator's `release`
  profile, no cache, into [`operator/Dockerfile`](../operator/Dockerfile). Each architecture's
  image is pushed untagged, by digest, and pulled back; nothing is tagged or signed until
  `tools/operator-image-check.mjs` has passed on exactly those bytes: Linux on that
  architecture, a numbered user who is not root, the operator as the whole default command,
  and a binary that starts there.
- **Tags:** `sha-<commit>`, and `main` for the image of the latest commit; `main` only moves
  forward. Tags can move, so name the image by its **digest**. The run's summary records it,
  and its `operator-image` artifact holds the digest and this commit's
  `operator/deploy/operator.yaml` with its image set to it.
- **Signed:** keyless with [cosign](https://docs.sigstore.dev/), by the `operator-image.yml`
  workflow on `main`, through GitHub's OIDC identity, as the runner image is: no key is stored
  anywhere, and the signature's entry in Sigstore's public transparency log names the image
  and its digest (ruled 2026-10-15). Check an image before you install it:

  ```bash
  cosign verify ghcr.io/purlis/purlis-cluster-operator@sha256:<digest> \
    --certificate-identity https://github.com/purlis/purlis/.github/workflows/operator-image.yml@refs/heads/main \
    --certificate-oidc-issuer https://token.actions.githubusercontent.com
  ```

## Before you install: doctor

`doctor` says what the cluster lacks for the cluster operator, one line per finding, each with the next
step. Run it as the admin who will install, with the cluster's context current in your
kubeconfig:

```bash
cargo run --manifest-path operator/Cargo.toml --release -- doctor
```

It names the cluster it asked (your context and its API server), then checks, in order:

- **Kubernetes 1.36 or later**, from the version the API server reports.
- **Upstream agent-sandbox v1.0.6**: its four definitions, each serving `v1beta1`, and its
  controller, `agent-sandbox-system/agent-sandbox-controller`, running the image tagged
  `v1.0.6` with a ready replica. Not installed yet is a finding too, whose next step is step 1
  below.
- **Your right to apply the install's cluster-scoped objects**: create and patch on
  namespaces, custom resource definitions, cluster roles and their bindings, and validating
  admission policies and their bindings, and `escalate` and `bind` on cluster roles, as
  `kubectl auth can-i` asks it. Without `escalate` and `bind`, Kubernetes refuses the install's
  cluster roles and their bindings unless you already hold every right the install's cluster
  roles grant, so doctor reports them missing.

It exits 1 when anything is missing and 0 when nothing is, so a script can gate the install on
it. It only reads: it gets the version, the definitions and the controller's deployment, and asks
the API server what you may do (`SelfSubjectAccessReview`), which the server answers and keeps
nothing of. What you may not read, or may not ask, is a finding naming the right to ask for.

```text
ok: asked context kind-team, API server https://127.0.0.1:6443/
ok: Kubernetes v1.36.4
ok: you may apply the install's cluster-scoped definitions
missing: upstream agent-sandbox v1.0.6: not installed. Next: apply it, step 1 of the install: kubectl apply --server-side -f operator/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml.
1 missing: install the cluster operator once doctor finds nothing missing
```

## Install

A Helm chart, [`operator/charts/purlis-cluster-operator/`](../operator/charts/purlis-cluster-operator/),
or the plain manifests in [`operator/deploy/`](../operator/deploy/), which are rendered from the
chart and make exactly the same objects: `operator/tests/the_chart.rs` holds the two equal, and
the kind tests install the chart and check that the plain manifests would change nothing. The
chart is not published to any chart repository; install it from a checkout of this repository.

Either way the operator's image is named by its **digest**: the chart refuses an install
without one, and the plain `operator.yaml` names a placeholder digest no registry holds until
you set the real one.

### With Helm

Helm 4, the version the tests use. Three steps, in order:

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller.
kubectl apply --server-side -f operator/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml

# 2. The release's namespace. Only pods that meet the restricted Pod Security Standard are
#    admitted in it; the operator's pod does.
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted

# 3. purlis's definitions, the policy on Claims, and the operator.
helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \
  --namespace purlis-system \
  --set image.digest=sha256:<digest> \
  --set 'imagePullSecrets[0].name=<a secret that pulls ghcr.io/purlis>'
```

**The definitions are a step of their own.** Helm installs the chart's `crds/` (Pool and
Claim) before anything else in it, and never upgrades or deletes them. A cluster admin who
reviews and applies them apart from the controllers applies `operator/deploy/crds.yaml` with
`kubectl apply --server-side` and installs the chart with `--skip-crds`. After an upgrade of
the chart that changes them, apply `operator/deploy/crds.yaml` the same way.

`helm uninstall` removes the operator and leaves the definitions, the Claims and Pools in
them, and the policy on Claims, which Helm keeps (`helm.sh/resource-policy: keep`).

The chart's values, all in its
[`values.yaml`](../operator/charts/purlis-cluster-operator/values.yaml), and
`values.schema.json` refuses any other:

| Value | What it sets | Default |
|---|---|---|
| `image.repository` | where the image is pulled from, without a tag or digest | `ghcr.io/purlis/purlis-cluster-operator` |
| `image.digest` | the image's digest, `sha256:` and 64 hex digits; required | none |
| `imagePullSecrets` | secrets in the release's namespace that pull the image | none |
| `resources` | the operator's CPU and memory | requests 10m CPU and 32Mi, limit 256Mi |

No value changes what the operator may do or how its pod is confined.

### With the plain manifests

The same steps with `kubectl`. The definitions and their policy are a step of their own, so a
cluster admin can review and apply them apart from the controllers.

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller.
kubectl apply --server-side -f operator/vendor/agent-sandbox/v1.0.6/sandbox-with-extensions.yaml

# 2. The policy that admits a Claim only for someone who may use its Pool, then purlis's
#    definitions, Pool and Claim. The policy goes first, so no Claim is admitted before it
#    holds.
kubectl apply --server-side -f operator/deploy/policy.yaml
kubectl apply --server-side -f operator/deploy/crds.yaml

# 3. The `purlis-system` namespace and the operator, with its image named by digest: the
#    `operator.yaml` of a run's `operator-image` artifact, which already names it, or this
#    one with the image set.
kubectl apply --server-side -f operator/deploy/operator.yaml
kubectl -n purlis-system set image deployment/purlis-cluster-operator \
  operator=ghcr.io/purlis/purlis-cluster-operator@sha256:<digest>
```

A build of your own, from [`operator/Dockerfile`](../operator/Dockerfile), is installed the
same way, with its own repository and digest.

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
  `PoolNotFound` when there is no such pool, `PoolDeleted` when it was deleted after the
  runner was asked for (below), `UpstreamObjectInTheWay` when an upstream object the operator
  did not make has the name it would use (below), `Starting` until upstream reports, and
  upstream's own reason after that. `kubectl get claims` shows it.
- **Deleting a Claim** deletes its runner.
- **Deleting a Pool stops nothing that runs.** A Claim whose runner was asked for keeps it,
  running as the pool last defined it, until the Claim is deleted: its `Ready` is still what
  upstream says of the runner, `True` while it runs, with the reason `PoolDeleted`. No new
  Claim is served from a deleted pool: one made after says `PoolNotFound`. A pool made again
  under the same name serves new Claims, and its definition is applied again to the template
  the kept runners came from.

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

**The operator acts only on Claims admitted under its policy, however it was installed.** It
reads the policy's binding before it acts on any Claim. While no binding puts the policy in
force, it acts on no Claim, and each says `PolicyNotInForce`. The policy is taken to be in force
30 seconds after its binding was made, the time every API server takes to enforce it. A Claim
made before then is never acted on: it says `MadeBeforePolicy`; delete it and make it again.
So wait 30 seconds after installing before making the first Claims.

Making the binding again, by reinstalling the policy or recreating the binding by hand, refuses
every Claim that already exists: each says `MadeBeforePolicy` and has to be deleted and made
again.

## What it makes

For a Claim in namespace `ns` on Pool `p`, the operator makes, and then applies server-side:

| Upstream object | Name | What it holds |
|---|---|---|
| `SandboxTemplate` | `purlis-pool-p` in `ns` | the runner's pod: one container running the pool's image with its own entrypoint (`purlis serve`), as a user who is not root, every capability dropped, no privilege escalation, the runtime's default seccomp profile, no Kubernetes credential mounted and no service addresses in its environment |
| `SandboxWarmPool` | `purlis-pool-p` in `ns` | no runner kept warm: every claim starts one |
| `SandboxClaim` | the Claim's own name in `ns` | the claim on that warm pool |

The upstream claim belongs to the Claim, and the template and the warm pool in a namespace to
the pool's Claims there, each Claim adding itself, so the cluster's garbage collector removes the
upstream claim with its Claim and the template and warm pool with the last of their Claims, and
never while a Claim in the namespace still has a runner from them. A Claim made as the last one
goes is served all the same: if the garbage collector removes the template or warm pool it has
just joined, the operator makes them again at once. Upstream's controller makes the pod
from the template and writes the upstream claim's status, which the operator reads.

**It changes only objects it made.** Before it writes any of the three, it reads what the
cluster holds under that name. An upstream claim it made names its Claim as its controller, and
a template or warm pool it made is owned by a Claim; any other object under that name, one made by hand or by another tool, is left
exactly as it is. The Claim then says `UpstreamObjectInTheWay`, naming the object's kind and
name, and nothing is made for it. Remove or rename that object and the Claim goes on at once: the
operator watches all three kinds.

## What it may do

Its ClusterRole, in [`operator/deploy/operator.yaml`](../operator/deploy/operator.yaml), is all
it holds:

| API group | Resources | Verbs |
|---|---|---|
| `runners.purlis.dev` | `pools` | get, list, watch |
| `runners.purlis.dev` | `claims` | list, watch |
| `runners.purlis.dev` | `claims/status` | patch |
| `admissionregistration.k8s.io` | `validatingadmissionpolicybindings`, only `purlis-claims-use-their-pool` | get, list, watch |
| `extensions.agents.x-k8s.io` | `sandboxclaims`, `sandboxtemplates`, `sandboxwarmpools` | create, get, list, patch, watch |

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

- `cargo test` in `operator/`, with `helm` 4 on PATH: what a Claim asks of upstream, that
  every field the operator writes is one the pinned upstream declares, what doctor finds on
  clusters the tests describe (and that it only reads), that the operator acts only on Claims
  admitted under its policy, the install manifests, and the chart, rendered and linted by Helm
  itself: the plain manifests are what it renders, its image is named only by digest, and no
  value changes what the operator may do. The definitions are generated from the Rust types and
  the plain manifests from the chart; regenerate them, in this order, with
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests` and
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart`.
- [`operator/e2e/kind.sh`](../operator/e2e/kind.sh) `<runner image> <operator image>`: a kind
  cluster of Kubernetes 1.36 with a registry beside it, so images are named by digest; doctor,
  which must name upstream agent-sandbox as missing; the pinned upstream, after which doctor
  must find nothing missing; purlis's definitions, the policy and the operator installed by the
  chart in a namespace that enforces the restricted Pod Security Standard; `kubectl diff`
  showing the plain manifests would change nothing the chart made; then
  `operator/tests/on_a_kind_cluster.rs`, which checks only what the cluster shows; then
  `helm uninstall`, which leaves the definitions and the policy. It removes everything it
  started. `.github/workflows/operator.yml` runs it, with a runner image built from the same
  commit, on every pull request that changes `operator/` or `runner-image/`.

## Moving the upstream pin

Upstream releases weekly. To move to another release: vendor its
`sandbox-with-extensions.yaml` under `operator/vendor/agent-sandbox/<version>/` in place of the
old one, set `VERSION` in `operator/src/upstream.rs` and the path in `operator/e2e/kind.sh`,
and run the tests. `only_what_the_pinned_upstream_defines_is_written` fails when a field the
operator writes is gone or renamed, before a cluster does.
