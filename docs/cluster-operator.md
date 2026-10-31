# The cluster operator

purlis's **cluster operator** is purlis in your cluster: a controller a team installs in its
own Kubernetes cluster, self-hosted and free (spec #2041). A team admin defines a **pool**
once: where the team's runners come from. A person asks for a runner from it with a
**claim**. The operator turns each claim into a runner, a pod running `purlis serve`, the same
session host as the person's own machine.

It is built on [kubernetes-sigs/agent-sandbox](https://github.com/kubernetes-sigs/agent-sandbox),
pinned to one tested release whose definitions purlis vendors, and it never re-implements what
that project does. Its source is [`operator/`](../operator/).

A Pool and a Claim make a running session host (#2054), the Claim says whether it is ready,
and the operator vouches for the runner's link key, so a desktop links only to the runner it
claimed (#2056, [below](#who-the-runner-is)). Each person's runners run in a namespace the
operator makes for that person alone, within a quota (#2055,
[below](#each-persons-namespace)).

## What it needs

- **Kubernetes 1.36 or later.** The operator's tests run on kind's Kubernetes 1.36.4.
- **Upstream agent-sandbox v1.0.6**, the release purlis is tested against. Its manifest,
  controller and definitions together, is vendored verbatim in
  [`operator/vendor/agent-sandbox/v1.0.6/`](../operator/vendor/agent-sandbox/v1.0.6/), with a
  kustomization beside it that names the controller's image by the digest of the release, so a
  tag moved in the registry never changes what runs. Install it with
  `kubectl apply --server-side -k`, as below.
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
  forward. Tags can move, so name the image by its **digest**. The run's summary records it.
  Its `operator-image` artifact holds the digest and `install/`: this commit's plain
  manifests and chart, with `operator.yaml`'s image and the chart's default `image.repository`
  and `image.digest` set to that image (#2281). So an install from it names the image built
  from the same commit as the manifests, with nothing to set.
  `tools/operator-install.mjs` makes it, and the kind tests install from what it makes.
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
  controller, `agent-sandbox-system/agent-sandbox-controller`, running the release's image by
  its digest with a ready replica. The same release by its tag alone, or at another digest, is
  a finding whose next step is to apply it by digest. Not installed yet is a finding too, whose
  next step is step 1 below.
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
missing: upstream agent-sandbox v1.0.6: not installed. Next: apply it, step 1 of the install: kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6.
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

Helm 4, the version the tests use. Four steps, in order:

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller, named by
#    the release's digest.
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6

# 2. The release's namespace. Only pods that meet the restricted Pod Security Standard are
#    admitted in it; the operator's pod does.
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted

# 3. The key the operator signs runners' link keys with, made once, here, and kept as a Secret
#    the operator's pod mounts. Keep it: to change it, roll it (see "Rolling the signing key").
#    Piped, so the key is never written to a file.
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin

# 4. purlis's definitions, the policy on Claims, and the operator.
helm install purlis-cluster-operator operator/charts/purlis-cluster-operator \
  --namespace purlis-system \
  --set image.digest=sha256:<digest> \
  --set 'imagePullSecrets[0].name=<a secret that pulls ghcr.io/purlis>'
```

**The definitions are a step of their own.** Helm installs the chart's `crds/` (Pool and
Claim) before anything else in it, and never upgrades or deletes them. A cluster admin who
reviews and applies them apart from the controllers applies `operator/deploy/crds.yaml` with
`kubectl apply --server-side` and installs the chart with `--skip-crds`. After an upgrade of
the chart that changes them, apply `operator/deploy/crds.yaml` the same way, before the chart:
Helm never upgrades `crds/`. An upgrade to a chart whose policy asks for a Claim's `requester`
without the definitions that hold it refuses every new Claim until they are applied.

`helm uninstall` removes the operator and leaves the definitions, the Claims and Pools in
them, and the policy on Claims, which Helm keeps (`helm.sh/resource-policy: keep`). A Claim
deleted while no operator runs waits for its finalizer, `runners.purlis.dev/runner`
([below](#each-persons-namespace)): delete the Claims before you uninstall, or, after, take the
finalizer off by hand (`kubectl patch claim <name> -n <namespace> --type=json
-p '[{"op":"remove","path":"/metadata/finalizers"}]'`) and delete the runner namespaces
(`kubectl delete namespace -l app.kubernetes.io/managed-by=purlis-cluster-operator`).

The chart's values, all in its
[`values.yaml`](../operator/charts/purlis-cluster-operator/values.yaml), and
`values.schema.json` refuses any other:

| Value | What it sets | Default |
|---|---|---|
| `image.repository` | where the image is pulled from, without a tag or digest | `ghcr.io/purlis/purlis-cluster-operator` |
| `image.digest` | the image's digest, `sha256:` and 64 hex digits; required | none |
| `imagePullSecrets` | secrets in the release's namespace that pull the image | none |
| `resources` | the operator's CPU and memory | requests 10m CPU and 32Mi, limit 256Mi |
| `forge.keySecretNames` | the Secrets in the release's namespace holding a GitHub App's key, which the Role `purlis-forge-key-reader` lets the operator `get` ([below](#minting-github-app-tokens-in-the-cluster)); empty, there is no such Role and it may read no Secret. The one value that changes what the operator may do, and never its own signing key | `[]` |
| `perPerson.quota` | what each person's runners may hold at once, in their namespace's quota `purlis-person` ([below](#each-persons-namespace)): `claims` (a count), `cpu` and `memory` (what their runners' pods ask for), `storage` (what their kept runners' volumes ask for); a key left out is not limited | `claims: 5`, `cpu: "4"`, `memory: 8Gi`, `storage: 50Gi` |

No value but `forge.keySecretNames` changes what the operator may do, and none changes how its pod
is confined.

### With the plain manifests

The same steps with `kubectl`. The definitions and their policy are a step of their own, so a
cluster admin can review and apply them apart from the controllers.

```bash
# 1. Upstream agent-sandbox, the pinned release: its definitions and its controller, named by
#    the release's digest.
kubectl apply --server-side -k operator/vendor/agent-sandbox/v1.0.6

# 2. The policies: the one that admits a Claim only for someone who may use its Pool, the
#    one on runner namespaces, and the one that narrows what the operator may change of
#    upstream's sandboxes (below); then purlis's definitions, Pool and Claim. The policies go
#    first, so no Claim is admitted before they hold.
kubectl apply --server-side -f operator/deploy/policy.yaml
kubectl apply --server-side -f operator/deploy/crds.yaml

# 3. The `purlis-system` namespace and the key the operator signs runners' link keys with,
#    kept as a Secret its pod mounts. Keep it: to change it, roll it (see "Rolling the signing key").
kubectl create namespace purlis-system
kubectl label namespace purlis-system pod-security.kubernetes.io/enforce=restricted
#    Piped, so the key is never written to a file.
openssl genpkey -algorithm ed25519 |
  kubectl -n purlis-system create secret generic purlis-cluster-operator-signing-key \
    --from-file=key.pem=/dev/stdin

# 4. The operator, with its image named by digest: the `install/operator.yaml` of a run's
#    `operator-image` artifact, which already names it, or this one with the image set.
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
  keptVolume:              # optional; these are the defaults
    size: 10Gi
    # storageClassName: fast   # the cluster's default class unless named
  stopped: false           # optional; true suspends every runner of the pool
  warmReplicas: 0          # optional; runners kept warm for fresh claims, per person
  idleAfter: 30m           # optional; these are the defaults (#3737)
  deleteSuspendedAfter: 720h
  noticeBeforeDelete: 168h
  maxAge: 336h
  # forge:                 # optional; see "Minting GitHub App tokens in the cluster"
  #   appKeySecretRef: { name: org-app }
  #   repos: [acme/*]
  #   mintInCluster: true
---
apiVersion: runners.purlis.dev/v1alpha1
kind: Claim
metadata:
  name: ada-1
  namespace: ada
spec:
  pool: team
  requester: ada@example.com
  kept: true               # optional; false (fresh) unless set
  stopped: false           # optional; true suspends this claim's runner
```

- **A Pool** is cluster-wide, defined once for the team. Its `image` is a runner image named
  by digest (`<repository>@sha256:<digest>`); the cluster refuses an image named by tag, since
  a tag can move. Its `keptVolume` is what each kept Claim's runner gets of its own: `size`
  (`Mi`, `Gi` or `Ti`, 10Gi unless set) and `storageClassName` (the cluster's default class
  unless set). A change to it applies to kept Claims made after it: a kept Claim keeps the
  volume it was made with. A change to it, as to `image`, also changes the pool's template, so
  a fresh Claim's running runner made before it is no longer vouched for; make its Claim again.
  A runner kept warm from the earlier template is replaced by one of the new (#2058): upstream
  deletes it, and a fresh Claim never takes it.
- **A Pool's warm count and times** (#3737). `warmReplicas` is how many runners the pool keeps
  warm for fresh Claims, 0 unless set and 20 at most. It applies in each person's runner
  namespace where that person has a fresh Claim on the pool that is not stopped, and never
  while the pool is `stopped`. The operator keeps each warm pool at that count, so a warm pool
  scaled by hand is brought back to it. **Warm runners count toward each person's quota**: set
  `perPerson.quota` to hold them as well as the person's own runners. A fresh Claim on a pool
  that keeps runners warm takes one already counted. A kept Claim is still refused once warm
  runners fill the quota. The times are a whole number followed by `s`, `m`
  or `h`, as a Docker pool's `suspendAfter` is (`1d` is refused; write `24h`):
  - `idleAfter` (30m, a minute at least): how long a paired runner is idle before it is suspended,
    as `stopped` suspends: its pod ends and a kept Claim's volume stays (#3980). Idle is no chat
    (a live chat waiting on the person counts as running), no browser (one holding an Ask among
    them), and no link holding a project. The runner is the only one that knows, so it says so
    by ending itself with the termination message `purlis-runner-idle`, which the operator reads
    from the pod's status. It needs no new right, and a runner image older than this never says
    it, so it is never suspended. The Claim's `Ready` is then `False` with the reason `Idle`, its
    `Idle` condition is `True`, and the next chat its desktop starts there runs it again: the
    desktop writes that condition's time in the annotation `runners.purlis.dev/run-again`, which
    you can do by hand too. A runner that says it is idle waits up to five minutes, serving
    nothing, for the operator to suspend it, and serves again if it does not. A change to
    `idleAfter` changes the pool's template, as a change to `image` does. It is a different
    clock from a runner left without a link, which a Docker pool's runner suspends itself after
    (its `suspendAfter`); a cluster pool's runner does not yet (#3703).
  - `deleteSuspendedAfter` (720h, 30 days): how long a kept Claim stays suspended before it is
    deleted, with its volume.
  - `noticeBeforeDelete` (168h, 7 days, shorter than `deleteSuspendedAfter`): how long before
    that deletion its requester is sent an Update.
  - `maxAge` (336h, 14 days): past it, a kept Claim's runner starts again on the pool's current
    image, and a fresh Claim's is made again.

  The cluster refuses any other spelling, a time too long to read, a notice that is not shorter
  than the deletion, and a warm count below 0 or above 20. **`deleteSuspendedAfter`,
  `noticeBeforeDelete` and `maxAge` are named, not yet acted on**: the operator deletes and
  restarts on them once #3981 (retention and its Update) and #3982 (maximum age) are built.
- **A Claim is kept or fresh** (#2057), and its `kept` never changes after it is made: the other
  kind is another Claim.
  - **A kept Claim** (`kept: true`) gets a runner of its own, started cold, whose home is a
    volume of its own: one PersistentVolumeClaim, `purlis-kept-<claim>`, holding the runner's
    checkout, conversation, profile sign-in, kept browser logins and its pairing. When its pod
    is lost, upstream makes it again under the same name on that volume, and the runner is the
    same runner, paired as it was (below). The volume goes when the Claim is deleted. Its
    pod and sandbox take the Claim's name, so keep a kept Claim's name to 63 characters.
  - **A kept Claim's home is made for it.** The first time its volume is used, the pod's first
    step writes the uid of the Claim's upstream claim beside the home (`/var/lib/purlis/claim`),
    and a later pod takes the home only when that uid is its own. A volume made or filled
    before its Claim was is refused, and the Claim has to be made again.
  - **A fresh Claim** (the default) takes a runner the pool keeps warm, when there is one, and
    keeps nothing past its pod: its home is a folder of the node's, as big as a kept Claim's
    volume.
  - purlis chooses: kept for a chat a person works in and for any chat of a subscription
    profile, whose sign-in lives on the volume; fresh for a short dispatched task. The person may
    turn a chat's "Keep this runner's work" either way.
- **A Claim** lives in a namespace and names its `pool`, which never changes: another pool is
  another claim. Its `Ready` condition says whether its runner is running, and if not, why:
  `PoolNotFound` when there is no such pool, `PoolDeleted` when it was deleted after the
  runner was asked for (below), `UpstreamObjectInTheWay` when an upstream object the operator
  did not make has the name it would use (below), `NamespaceInTheWay` when a namespace the
  operator did not make, or made for someone else, has the name of the requester's runner
  namespace (below), `OverQuota` when the runner would take the requester over their quota
  (below), and upstream's own reason otherwise. **While its runner starts**, `Ready` is `False`
  with its stage: `Scheduling` until a node takes the pod (with the scheduler's words when it
  cannot), `Pulling` while the node pulls the image (with the kubelet's words when it fails),
  `Starting` after that. A reason upstream gives for a runner that will not be ready on its
  own, such as an error or an expired claim, is shown as upstream gives it.
  `kubectl get claims` shows it.
- **A Claim names who makes it**, `requester`, exactly as the cluster signs them in
  (`kubectl auth whoami`): the policy refuses a Claim whose `requester` is anyone else, in words
  that name the right one, and it never changes after. The operator vouches for it with the
  runner's link key. purlis fills it in when it claims a runner.
- **`stopped` suspends, and never destroys** (#2058). Set on a Pool or a Claim from kubectl
  (`kubectl patch pool team --type merge -p '{"spec":{"stopped":true}}'`), it suspends every
  runner of the pool, or the Claim's runner: the operator puts each runner's upstream sandbox in
  upstream's `Suspended` mode, and upstream ends its pod and keeps its volumes, so a kept
  Claim's home, its work and its pairing stay. A fresh Claim's runner keeps nothing past its
  pod, as ever. The Claim's `Ready` is `False` with the reason `Stopped`, saying whether the
  Claim or its pool is stopped and whether its runner is being suspended or is suspended
  (upstream's own `Suspended` condition), and no key is vouched for. While its runner is being
  suspended the Claim is looked at again less and less often; after about four and a half
  minutes it says upstream has not ended the runner's pod yet. A Claim made while stopped gets
  no runner yet. A stopped pool keeps no runner warm. A Claim's own `stopped` stops its runner
  even after its Pool is deleted. With its Pool deleted a runner is never run again: one
  suspended by its pool's or its own `stopped` stays suspended, a kept Claim's volume kept,
  until the Claim is deleted, and its `Ready` says `PoolDeleted` and that it stays suspended.
  Set back to `false` (the default), each runner the operator suspended runs again: a kept Claim's pod is made again on its volume, paired as it
  was, with the same key. A runner suspended by hand before it was stopped is left suspended,
  as far as the operator can tell: it goes by who last applied the sandbox's mode, which any
  client may name itself as, and a runner suspended by hand while it was already stopped runs
  again with the rest. Once a stopped pool runs again, its warm pools keep its `warmReplicas`
  warm again.
- **A Claim stays in its namespace; its runner does not.** The runner, and everything the
  operator makes for it, is in the requester's own namespace, `purlis-runners-<hash>`
  ([below](#each-persons-namespace)).
- **Deleting a Claim** deletes its runner: the Claim carries the finalizer
  `runners.purlis.dev/runner`, and the operator deletes what it made for the Claim before it lets
  the Claim go.
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

**A runner from a pool is had only through a Claim.** The same policy refuses an upstream
`SandboxClaim` on one of a pool's warm pools, and an upstream `SandboxWarmPool` of one of a
pool's templates (both named `purlis-pool-<pool>`, below), when it is made there, moved there or
moved off one by anyone but the operator's service account: such a runner would skip the checks
above. Upstream's own objects that are not a pool's are left to upstream. This holds as far as
Kubernetes' usual boundary goes: whoever may act as the operator's service account (impersonate
it, mint its tokens, or run a pod as it in the operator's namespace), or change the policy and its
binding, passes it. Rights in the operator's namespace are the operator's rights.

## Who the runner is

A desktop links to a runner over ADR 0078's encrypted link, and links only to the runner it
claimed. Two witnesses say which key that runner holds, and both must agree:

1. **The runner makes its link key as its pod starts.** The pod's first step, `link-key`, runs
   the pool's image (`purlis bridge --make-key`) before anything else in the pod: it makes the
   key, hands its private half to the runner in a folder in memory the two containers alone
   share (`/run/purlis-link/`), and writes its public half as its termination message, which
   the cluster shows in the pod's status.
2. **The operator signs what it saw.** It follows the claim to its pod by uid: its upstream
   claim, the sandbox that claim controls, and the pod that sandbox controls. It holds the pod
   to the template it made for the pool in full, every field but the few the cluster itself
   sets or defaults (a kept Claim's pod with its own volume in place of the template's folder,
   as upstream puts it there), and refuses a pool's template that someone else changed. Then
   it signs a statement of the cluster, the claim (its namespace, name and uid), the runner
   namespace and the pod's name there, the key, the requester and until when it holds (ten
   minutes on) with its own Ed25519 key. The statement and its signature are the Claim's
   `status.linkKey`, and its `KeyPublished` condition says `Vouched`, or why not yet. Every
   Pool's `status.signingKey` names the operator's key and `status.cluster` its cluster, the uid
   of the cluster's `kube-system` namespace, which the operator reads once as it starts. A
   desktop pins both when it adds the pool.
3. **The desktop pairs over its own `kubectl exec`** into the pod the statement names, in the
   runner namespace it names, as a Docker pool's desktop pairs over `docker exec`, and the
   runner answers with its device id and key. A statement signed by another key, made in
   another cluster, about another claim or requester, naming a runner namespace other than the requester's own, or a runner
   answering another key than the one vouched for, is refused, and so is a statement that names
   another runner namespace, pod or key before any later link. The pairing takes the private half out of the
   shared folder, so it is left only in the session host's own. The operator looks at a
   vouched claim again every minute, and takes its statement away once it no longer holds.

**Vouching is only as strong as the runner namespace is the requester's own.** Anyone who may
exec into pods, make pods, or change the pool's objects in a runner namespace can reach its
runners, read their keys, or pair with them first, whatever the statement says. The operator
gives the requester alone a right there ([below](#each-persons-namespace)); grant no one else
rights in `purlis-runners-*` namespaces, cluster-wide rights on pods included. Read access to
Secrets in the operator's namespace is the power to vouch for any runner.

**Adding the pool in a person's window** (#3292): the add pool dialog (the palette's Add a Docker
pool…, or Add a pool… in Settings › This machine › Pools once one is kept), Kind Cluster, their
kubeconfig's context for the cluster and the Pool's name. Check its key shows
the operator key the Pool's status names; give your people that key to check it against, as

```bash
kubectl get pools.runners.purlis.dev <pool> --output jsonpath='{.status.signingKey}'
```

prints it. Add pool pins it and makes the person's one kept Claim, `<pool>-<a label of their
desktop>`, marked `runners.purlis.dev/desktop: <that label>`, in the namespace their context
names (`default` where it names none). Removing the pool deletes only that Claim. A Pool that
later names another key refuses every link to that desktop's runner, and the window asks the
person to check it and add the pool again.

A runner is paired once, and **its pairing is its Claim's, not its pod's** (#2057, ruled
2026-10-28). The pairing lives in the runner's home: a kept Claim's runner keeps it on its
volume, and when its pod is made again the pod's first step finds it and publishes the key the
runner was paired with, so the operator vouches for the same key and the desktop links to the
same runner without pairing again. A fresh Claim's runner keeps nothing past its pod: a new pod
has a new key, which the desktop refuses as a changed key, and the claim is made again.

**A kept Claim's volume holds its runner's link key.** Anyone who can read the volume, or make a
pod that mounts it, in the runner namespace can take the runner's identity, as anyone who may
exec into its pod can; so can whoever holds the node or the storage behind it, and so does any
snapshot or backup of the volume (Velero and the like), which holds a working link key for as
long as the Claim lives. The namespace rule above holds for the volume too. A statement holds
for ten minutes once signed, and the operator signs it again while it still vouches, so one
it can no longer look at again stops vouching within ten minutes. A desktop reads none that holds
more than fifteen minutes ahead of its own clock.

**Before a vault value or a token goes to a runner the person marked as their own**, the desktop
also asks the cluster that the Claim and the Pool are not stopped and that the Claim's
`KeyPublished` condition says `Vouched`, and reads the pod's uid, so a pod made again under its
name asks again for a browser login. None of those is signed, so they only ever refuse or ask
again; the signed statement is what vouches.

**The pod's group.** Every runner's pod is in group 10001 (`fsGroup`), which may write its
volume. Where the volume's root ever loses that group or mode (a restore, a storage
migration), the kubelet sets them again on every file, and the runner's key and peers file
become readable by its group: purlis then refuses them, and a kept runner has to be claimed
again. A cluster whose admission sets or changes a pod's security context (an OpenShift
security context constraint, a policy engine's mutation) refuses the pool's pods, or the
operator refuses to vouch for them, since they are no longer its template.

**The operator acts only on Claims admitted under its policy, however it was installed.** It
reads the policy's binding before it acts on any Claim. While no binding puts the policy in
force, it acts on no Claim, and each says `PolicyNotInForce`. The policy is taken to be in force
30 seconds after its binding was made, the time every API server takes to enforce it. A Claim
made before then is never acted on: it says `MadeBeforePolicy`; delete it and make it again.
So wait 30 seconds after installing before making the first Claims.

Making the binding again, by reinstalling the policy or recreating the binding by hand, refuses
every Claim that already exists: each says `MadeBeforePolicy` and has to be deleted and made
again. A runner such a Claim already has keeps running until the Claim is deleted, and its
finalizer still removes it then.

## Each person's namespace

**A person's runners run in a namespace of their own** (#2055, ruled 2026-10-29), whichever
namespace their Claims are in. A person, here, is a username as the cluster signs it in: the
Claim's `requester`. Each username has a runner namespace and a quota of its own, a service
account's too. For each requester the operator makes, the first time one of their Claims is acted
on:

| Object | Name | What it holds |
|---|---|---|
| `Namespace` | `purlis-runners-<hash>` | the requester's runner namespace. `<hash>` is the first 16 hex digits of the SHA-256 of the requester, exactly as the Claim names them, so a desktop can work out the name itself. Labelled `app.kubernetes.io/managed-by: purlis-cluster-operator` and annotated `runners.purlis.dev/requester` with the requester: the operator uses a namespace of that name only when it carries both, for this requester, and the Claim otherwise says `NamespaceInTheWay` and waits |
| `RoleBinding` | `purlis-runner-owner` | the cluster role `purlis-runner-owner`, which the chart installs, for the requester alone: a `User`, or the `ServiceAccount` a `system:serviceaccount:<namespace>:<name>` requester names. The role allows `get`, `list` and `watch` on pods, `create` and `get` on `pods/exec`, and `get` on `pods/log`: what a desktop needs to pair with and link to the runner over `kubectl exec`, and to read its log. Nothing else: the requester makes, changes and deletes nothing there |
| `ResourceQuota` | `purlis-person` | the chart's `perPerson.quota`: `claims` as `count/sandboxclaims.extensions.agents.x-k8s.io`, `cpu` as `requests.cpu`, `memory` as `requests.memory` and `storage` as `requests.storage`. Applied again whenever it differs from the chart's value, so a changed value reaches each person as their Claims are next looked at |

**The policy on runner namespaces holds them at admission.** A second ValidatingAdmissionPolicy,
`purlis-runner-namespaces`, installed beside the policy on Claims (`operator/deploy/policy.yaml`)
and kept when the release is uninstalled, as that one is:

- a namespace named `purlis-runners-*` is made only by the operator's service account, and no
  one else changes its `app.kubernetes.io/managed-by` label or `runners.purlis.dev/requester`
  annotation, through the namespace or its `status` and `finalize`;
- the operator makes no other namespace;
- the operator makes or changes a role binding only in a runner namespace, only
  `purlis-runner-owner` of the cluster role `purlis-runner-owner`, and only for one subject: the
  person the namespace's `runners.purlis.dev/requester` names, as a user whose name does not
  start `system:` or as the service account a `system:serviceaccount:<namespace>:<name>`
  requester names, and never the operator itself;
- the operator makes or changes a quota only in a runner namespace, and only `purlis-person`.

The operator uses a runner namespace only while that policy's binding is in force, and only one
made after it was, 30 seconds after the binding was made, as with Claims. It makes one only then,
too: a Claim it meets sooner, as on the upgrade that brings the policy, waits until then with
nothing made. Its other labels, annotations and owners are a namespace's own, which anyone who
may change namespaces may change, as with any namespace. While no binding puts
it in force, a Claim says `PolicyNotInForce`, naming the policy. A runner namespace made before
says `NamespaceInTheWay`; making the binding again does that to every runner namespace there is,
so a cluster admin then deletes them, with the runners in them, and their people's Claims are
made again.

**The Claim stays where it is made.** Its runner, and the upstream objects below, are in the
runner namespace; the Claim's status, its statement included, stays on the Claim. Two Claims of
one person in two namespaces, of the same name, would make upstream claims of the same name: the
second says `UpstreamObjectInTheWay` until the first is deleted.

**The quota.** Every runner's pod asks for what its containers need at least, 250m CPU and 512Mi
for the runner and 50m and 32Mi for its first step, so the quota can count it, and a kept
Claim's volume asks for its Pool's `keptVolume.size`. Before the operator asks upstream for a
new runner, it reads the quota's use: when the runner would take any of the four over, the Claim
says `Ready` `False` with the reason `OverQuota`, naming what is over, and nothing is made for
it. It is looked at again every 30 seconds, and goes on once one of the person's other Claims is
deleted or the quota is raised. A request the cluster refuses for the quota says `OverQuota`
too. Runners a Pool keeps warm in a person's namespace count toward their quota. A runner whose
pod or volume the cluster refuses later, made at the same moment as another, says upstream's
reason.

**The finalizer.** Each Claim the operator acts on carries the finalizer
`runners.purlis.dev/runner`. When the Claim is deleted the operator deletes its upstream claim,
which takes its runner and a kept runner's volume with it, and, when no other Claim of the
requester on that pool is left, the pool's template and warm pool in the runner namespace; then
it takes the finalizer off and the Claim goes. The runner namespace, its role binding and its
quota stay, empty, for the person's next Claim: the operator holds no right to delete a
namespace.

## What it makes

For a Claim on Pool `p` by a requester whose runner namespace is `r`, the operator makes, and
then applies server-side:

| Upstream object | Name | What it holds |
|---|---|---|
| `SandboxTemplate` | `purlis-pool-p` in `r` | the runner's pod: its first step, `link-key`, making the runner's home and its link key (above), then one container running the pool's image with its own entrypoint (`purlis serve`), both as a user who is not root, every capability dropped, no privilege escalation, the runtime's default seccomp profile, no Kubernetes credential mounted and no service addresses in their environment, each asking for the CPU and memory above. The runner's `HOME` is `/var/lib/purlis/home`, on the volume `purlis-kept` mounted at `/var/lib/purlis`: a folder of the node's in the template, which a kept Claim's own volume takes the place of. The pod's group (`fsGroup` 10001) may write that volume. A claim may add a volume of its own (`volumeClaimTemplatesPolicy: Allowed`) |
| `SandboxWarmPool` | `purlis-pool-p` in `r` | the Pool's `warmReplicas` runners kept warm for that person's fresh Claims, none while the pool is `stopped`; scaled by hand, it is brought back to that count (#3737). Its update strategy is upstream's `Recreate`, so a warm runner made from an earlier template is deleted and made again, and never handed to a fresh Claim (#2058) |
| `SandboxClaim` | the Claim's own name in `r` | the claim on that warm pool, annotated `runners.purlis.dev/claim-uid` with the Claim's uid; a kept Claim's carries the volume `purlis-kept` its Pool's `keptVolume` asks for, so upstream makes its runner cold and its PersistentVolumeClaim `purlis-kept-<claim>` |

Each of the three is controlled by the runner namespace, by its uid: a Kubernetes owner cannot be
in another namespace than what it owns, so the Claim is named by the annotation instead, and the
finalizer removes them (above). Upstream's controller makes the pod from the template and writes
the upstream claim's status, which the operator reads.

**It changes only objects it made.** Before it writes any of the three, it reads what the
cluster holds under that name. An object it made is controlled by the runner namespace it made,
by uid, and an upstream claim it made also names its Claim's uid; any other object under that
name, one made by hand or by another tool, is left exactly as it is. The Claim then says
`UpstreamObjectInTheWay`, naming the object's kind, name and namespace, and nothing is made for
it. Remove or rename that object and the Claim goes on at once: the operator watches all three
kinds.

**After an upgrade from a version without runner namespaces**, a Claim made before keeps the
runner it had in its own namespace, as well as getting a new one in its requester's runner
namespace, which its desktop links to. The old runner is outside that namespace and outside the
person's quota, and anyone with rights in the Claim's namespace can still reach it. Delete such
Claims and make them again soon after upgrading: the old runner goes with the Claim.

## Its metrics

The operator serves its metrics in Prometheus's text format on its pod's port `metrics`
(8080), at `/metrics`, and the chart and the plain manifests give that port a Service of its
own, `purlis-cluster-operator-metrics` in the operator's namespace (#3737). Point your
Prometheus, or a `ServiceMonitor`, at it:

| Metric | Type | Labels | What it counts |
|---|---|---|---|
| `purlis_cluster_operator_claims` | gauge | `pool`, `namespace`, `state` | the Claims the operator watches; `state` is `Ready` when the runner is ready, otherwise the Claim's `Ready` reason (`Stopped`, `Idle`, `Starting`, `OverQuota`, …), and `Unknown` before the operator has said one |
| `purlis_cluster_operator_warm_runners` | gauge | `pool`, `namespace` | the runners upstream says are warm and ready in each runner namespace |
| `purlis_cluster_operator_suspensions_total` | counter | `pool`, `cause` | the runners the operator suspended; `cause` is `stopped` (the Claim's or its Pool's `stopped`) or `idle` (nothing ran on it for its pool's `idleAfter`) |
| `purlis_cluster_operator_claims_deleted_total` | counter | `pool`, `cause` | the Claims whose runner the operator removed as they were deleted; `cause` is `requested` (someone deleted the Claim) |

The counters count from the operator's start. The causes grow as the operator deletes
long-suspended Claims (#3981).

**Nothing may scrape them until you say who may.** The metrics name every namespace that holds
a Claim and every runner namespace (`purlis-runners-<hash>`, one per person), with their
Claims' states, so they are not for everyone in the cluster. They say nothing about what a
runner does. The NetworkPolicy `purlis-cluster-operator` lets nothing reach the metrics port
but the scrapers you name. With the chart, name them as a NetworkPolicy's
`from` peers:

```sh
helm upgrade purlis-cluster-operator operator/charts/purlis-cluster-operator \
  --namespace purlis-system --reuse-values \
  --set 'metrics.allowFrom[0].namespaceSelector.matchLabels.kubernetes\.io/metadata\.name=monitoring'
```

With the plain manifests, add a NetworkPolicy of your own in `purlis-system` that lets your
Prometheus's pods reach the operator's pod (`app.kubernetes.io/name: purlis-cluster-operator`)
on port `metrics`. NetworkPolicies add up, so it opens only what it names. The policy takes
effect where the cluster's network plugin enforces NetworkPolicies. The port holds at most 32
connections at once and closes one that has not sent its request within two seconds.

## Rolling the signing key

A team admin rolls the operator's signing key without any desktop being asked again (#3305).
The key and the next one are two files of the one Secret, `purlis-cluster-operator-signing-key`:
`key.pem`, the key it signs with, and `next.pem`, there only during a roll. The operator's pod
mounts the whole Secret, and the kubelet refreshes it when it changes, so a roll needs no
restart: the operator reads both files again each time it signs a statement or looks at a Pool,
both from the one version of the Secret the kubelet holds then.

1. **Add the next key** as `next.pem`, piped, so it is never written to a file:

   ```sh
   kubectl -n purlis-system patch secret purlis-cluster-operator-signing-key --type merge \
     -p "{\"data\":{\"next.pem\":\"$(openssl genpkey -algorithm ed25519 | base64 | tr -d '\n')\"}}"
   ```

   Within a couple of minutes every Pool's status names it as `status.nextSigningKey`, with
   `status.nextSigningKeyVouch`: a statement, `purlis cluster operator key roll v1`, of the
   cluster, the key it rolls from, the key it rolls to and until when it holds, signed by the
   current key. It holds seven days, and the operator signs it again while `next.pem` is there.
   Every Claim's statement is signed by both keys: `status.linkKey.signature` by the current
   key, `status.linkKey.nextSignature` by the next.
2. **Wait while desktops see it.** A desktop that reads the Pool during the roll checks the
   current key's word with the key it pinned, and moves its pin to the next key, asking no one.
   Keep `next.pem` for as long as your desktops may go without looking at the pool: a week is
   a good default.
3. **Make the next key the key**: `next.pem` becomes `key.pem`, and `next.pem` goes, in one
   change, so the kubelet swaps both at once:

   ```sh
   kubectl -n purlis-system get secret purlis-cluster-operator-signing-key -o json |
     jq '.data["key.pem"] = .data["next.pem"] | del(.data["next.pem"])' |
     kubectl replace -f -
   ```

   Every Pool names the new key alone, and every statement is signed by it alone. A desktop
   that missed the roll refuses the pool's new key and asks for the pool to be added again, as
   for any key it did not pin. The old key signs nothing now; delete any copy of it.

**A roll given up** (`next.pem` removed without step 3) leaves desktops that had moved their pin
pinned to a key the operator no longer uses: they ask again. Roll forward rather than back.

## What it may do

Its ClusterRole, in [`operator/deploy/operator.yaml`](../operator/deploy/operator.yaml), is all
it holds, but for the one Role below where the chart names a key:

| API group | Resources | Verbs |
|---|---|---|
| `runners.purlis.dev` | `pools` | get, list, watch |
| `runners.purlis.dev` | `claims` | get, list, watch, patch (its finalizer) |
| `runners.purlis.dev` | `claims/status` | patch |
| `runners.purlis.dev` | `pools/status` | patch |
| `authorization.k8s.io` | `subjectaccessreviews` | create (may a Claim's requester still use its Pool, before each mint) |
| `networking.k8s.io` | `networkpolicies` | create; get and patch only `purlis-forge-mint` |
| core | `pods` | get |
| core | `namespaces` | create, get (`kube-system` too, for the cluster's uid) |
| core | `resourcequotas` | create; get and patch only `purlis-person` |
| `rbac.authorization.k8s.io` | `rolebindings` | create; get and patch only `purlis-runner-owner` |
| `rbac.authorization.k8s.io` | `clusterroles`, only `purlis-runner-owner` | bind |
| `agents.x-k8s.io` | `sandboxes` | get, patch |
| `admissionregistration.k8s.io` | `validatingadmissionpolicybindings`, only `purlis-claims-use-their-pool`, `purlis-runner-namespaces` and `purlis-forge` | get, list, watch |
| `admissionregistration.k8s.io` | `validatingadmissionpolicies`, only `purlis-forge` | get |
| `extensions.agents.x-k8s.io` | `sandboxclaims`, `sandboxtemplates`, `sandboxwarmpools` | create, delete, get, list, patch, watch |

It holds no right to `exec`, `attach` or port-forward to a pod itself, and in that ClusterRole
none to read a Secret. Beside it, only where the chart's `forge.keySecretNames` names any, the
Role `purlis-forge-key-reader` in the operator's own namespace gives it `get` on the Service
`purlis-cluster-operator-mint`, whose address a Claim's status says, and on those Secrets
alone ([below](#minting-github-app-tokens-in-the-cluster)).
It binds one cluster role, `purlis-runner-owner`, only to one person, only in runner namespaces:
RBAC cannot say where a role may be bound, so the policy on runner namespaces (above) holds it.
It never deletes a namespace. It reads a sandbox and a pod only by name: a Claim's, which
upstream made, to follow the claim to its runner and read the key it vouches for. The pod is the
one the sandbox records: upstream's `agents.x-k8s.io/pod-name` annotation where it carries one
(a runner adopted from a warm pool before upstream made warm runners whole sandboxes), the
sandbox's own name otherwise, and still held to being the pod that sandbox controls. It never
lists, makes, changes or deletes a pod itself, though suspending a sandbox has upstream end its
pod. Its own signing key is the Secret you made at install, which the kubelet mounts in its pod.
Its Kubernetes client is built
without the websocket support `exec` would need.
`operator/tests/the_install_manifests.rs` holds these rules, and the kind tests ask the cluster
itself. It runs as a user who is not root, with a read-only root file system and every
capability dropped.

**Its `patch` on sandboxes is narrowed by the cluster** (#2058). RBAC cannot limit a right to
one field or to some owners: as a role, `patch` on `sandboxes` is every field of every Sandbox
in the cluster, those other users of agent-sandbox made without any SandboxClaim included. The
operator needs only one: to suspend a stopped Claim's runner and run it again, it applies
`spec.operatingMode` and nothing else, to the sandbox its Claim's upstream claim controls, with
that sandbox's uid as a precondition. Another admission policy in `policy.yaml`,
`purlis-operator-changes-only-a-sandboxs-mode`, makes that the cluster's rule: an update by the
operator's service account to a Sandbox is refused unless a SandboxClaim controls it (any
SandboxClaim's, purlis's or not), and unless every field of its spec but `operatingMode`, and
its labels, annotations, owners and finalizers, are as they were. The operator holds no right
on admission policies or their bindings but reading the bindings of the policies on Claims and
on runner namespaces, so it cannot lift it. The spec fields it holds as they were are the
pinned upstream's (v1.0.6, and a test holds the list to the vendored definitions): a cluster
that serves another agent-sandbox version may have fields the policy does not hold, one more
reason to run the version `doctor` checks for.

Upstream's controller holds its own ClusterRoles, in the vendored manifest: it makes and
removes the runners' pods, services, volume claims and network policies. It holds no Secret
right and no `exec` right either.

**Cluster admins and node owners can read everything in the cluster**, every runner's files
and memory included. Give cluster-admin and node access accordingly.

## Minting GitHub App tokens in the cluster

**Not finished yet.** The operator mints for a vouched runner that asks it (below), but no chat
asks until #4207: a chat on a pool still has no token minted here.

A cluster admin opts a Pool in with these steps:

```sh
# 1. The organisation's GitHub App, as a Secret in the operator's namespace: `app` is the App's
#    client id and `key` its private key, as a PEM.
kubectl -n purlis-system create secret generic org-app \
  --from-literal=app=<the App's client id> --from-file=key=<the App's private key>.pem

# 2. A cluster installed before these keys needs the new definitions first (Helm leaves them).
kubectl apply --server-side -f operator/deploy/crds.yaml

# 3. The chart names it, which is all the operator may read: the Role purlis-forge-key-reader
#    gets `get` on these names and no others, in this namespace.
helm upgrade purlis-cluster-operator <chart> --namespace purlis-system --reuse-values \
  --set 'forge.keySecretNames[0]=org-app'
```

```yaml
# 4. The Pool says so.
apiVersion: runners.purlis.dev/v1alpha1
kind: Pool
metadata:
  name: team
spec:
  image: ghcr.io/purlis/purlis-runner@sha256:<digest>
  forge:
    appKeySecretRef: { name: org-app }  # required; in the operator's namespace, it says no other
    repos: [acme/*, other/app]          # required; owner/name or owner/*
    mintInCluster: true                 # false unless set
```

- **A plain install has no such Role.** `operator/deploy/` is rendered with no name set, so an
  admin installing from it applies the Role and its RoleBinding themselves, as
  `helm template --set 'forge.keySecretNames[0]=<name>'` renders them. With no name set there is no
  Role at all, since an empty `resourceNames` would mean every Secret.
- **The operator reads a Secret only for a Pool that says `mintInCluster: true`**, by name, and
  says in its log, within a minute, whether that Pool's key can be read and which App it is, or why not: not named
  by the chart, not there, or not holding both keys. It logs neither the key nor anything else of
  the Secret. It does not keep the key.
- **`Pool.spec.forge.repos` is the most a token may cover.** A token covers only the repos that
  are on this list and also on the Claim, compared without regard to case. A bare `*`, `*/*`, a partial wildcard, a path or a URL
  is refused at admission. Keep the list as narrow as the team's work: anyone who may `use` the
  Pool can reach these repos through it.
- **Writing a Pool's `spec.forge` takes `use` on the key it names.** The policy `purlis-forge`
  refuses the write otherwise. Grant it by the Secret's name; `forgekeys` names a right, and the
  cluster holds no such object:

  ```yaml
  - apiGroups: [runners.purlis.dev]
    resources: [forgekeys]
    resourceNames: [org-app]
    verbs: [use]
  ```

  On a Pool that mints, changing `spec.image` takes the same right, since the image holds the link
  keys the operator vouches for. Taking `spec.forge` off a Pool asks nothing. A wildcard RBAC rule
  on `runners.purlis.dev` holds every key. The right is asked as a Pool is written: revoking it stops
  further writes, and to stop a Pool already written, change or delete the Pool.
- **`Claim.spec.forge.repos`** names the repos of the workspaces the desktop sends, each exactly
  `owner/name`: a wildcard is refused at admission. As a chat starts on the runner the desktop adds
  the github.com repos of that chat's workspace, as who the kubeconfig signs in as, and the runner's
  other chats keep theirs: **a token covers every repo on the Claim that the Pool allows, whichever
  chat asks.** Only the Claim's `requester` may change
  it, while they may `use` its Pool, as the policy holds. Someone else who may patch claims in the
  namespace cannot.
- **A Pool never names `purlis-cluster-operator-signing-key`**, and neither do the chart's values.
- **A tool line names one App.** A persona's tool whose `app` is not the Secret's `app` is refused
  by the operator.
- **The desktop reads `mintInCluster` from the Pool** as the pool is added, and records it
  ([`runners.json`](plane-format.md)); the person does not choose it. To pick up a change, remove
  the pool and add it again, which also releases its claim and runner. The operator goes by the Pool as it is now.

### How a runner gets a token

The chart adds the Service `purlis-cluster-operator-mint` (port 8443, TLS 1.3) only where
`forge.keySecretNames` names a key, and the NetworkPolicy `purlis-cluster-operator` then lets the
namespaces the operator made for runners reach that port. In a runner namespace with a Claim on an
opted-in Pool, the operator makes the NetworkPolicy `purlis-forge-mint`, for every pod of that
namespace and for as long as it stands, whose one rule is the way out to that port. Reaching the
port gets nothing without a proof and an opted-in Pool: NetworkPolicies add up, so upstream's own policy keeps every other limit on the
runner. Upstream gives a runner public name servers, so a runner reaches the port by the Service's
address, not its name. A plain install applies the Service and the NetworkPolicy as
`helm template --set 'forge.keySecretNames[0]=<name>'` renders them.

The port's certificate is self-signed by the operator's signing key, the key every Pool's status
names. No authority stands behind it, so a client holds its public key to that key.

Only the operator may put its `managed-by` label on a namespace, and only on a runner namespace.

A runner asks in two steps. `POST /forge/challenge` answers a challenge good for 60 seconds and one
use. A proof also names the operator's key the runner holds the port to. `POST /forge/token` takes `namespace`, `claim`, `app`, `challenge` and a `proof`: a MAC only the
holder of the runner's link key can make (`operator/src/mint.rs`). The operator then requires, each
read at that moment:

- a statement of its own on the Claim that still holds, for the key that made the proof, and the pod
  it names still there holding that key. Short of this every refusal is the same words, so the port
  says nothing of which claims there are;
- the Claim and the Pool made after the policy `purlis-forge` settled. **A Pool or Claim made before
  never mints: delete it and make it again**, since what it says was not held to that policy;
- the runner's pod running its Pool's image as it is now;
- the Pool opted in, neither stopped nor being deleted, and at least one repo on both lists, of one
  owner;
- **the requester still allowed `use` on the Pool**, asked of the cluster before each mint, so a
  revoked right stops new tokens at once. A Claim records no groups, so this is asked of the person
  by name: **a right held only through a group is refused. Grant `use` to the person by name** on a
  Pool that mints;
- the Secret's key parsed, its `app` the one asked for.

The token is limited to those repos by name and, also by name, to what the installation holds of
the repo-level permissions that administer nothing: `actions`, `checks`, `contents`, `deployments`,
`discussions`, `issues`, `metadata`, `pull_requests`, `repository_projects` and `statuses`, at
`write` at most. So a minted token cannot change administration, secrets or workflow files. It can
still push code that an existing workflow runs, with what that workflow may use, and set checks and
statuses: keep branch rules and environments in mind when you list repos. A token GitHub says covers anything else is revoked and not handed over.
It is not stored, and nothing about it is written to the API server. The operator's log has every
mint (who, which pool and App, the repos, the permission names, the expiry) and every refusal, with
no token and no key in it.

### What the opt-in means

**Anyone who may use the Pool, by name, can use its listed repos through it, with no one asked each
time.** On a
Pool that opts in, a chat's GitHub App tool (`mint = "github-app"`, whose App is the Secret's) gets
its token from the operator, not the desktop, and the person is not asked about that run: the admin's
opt-in stands as consent (ruled on #3807), and the operator's log is the record of each mint. This
holds for such tools alone. Any other tool's vault value still comes from the desktop, with its Ask.

- **On a Pool that does not opt in nothing changes:** the run needs the desktop and the person's Ask.
- **A refused mint is not the end of the run, and never a token by itself.** The chat is told the
  operator's words, and the run goes to the desktop, which gates it and asks the person as on any
  other pool. With the desktop away, it waits as such a run does.
- **What a run is held to on the runner is what its chat's start was armed with.** The desktop's
  gate is not asked: a tool line or vault tag taken away reaches a chat as it starts again. A token
  is kept about 50 minutes for a chat's next runs, so a right revoked stops the next mint, not one
  already out.
- **A chat here runs only inside purlis's sandbox.** A runner of a Pool that names an App's key is made
  with the variable `PURLIS_MINTING_POOL`, and from its first moment its host refuses any start outside
  the sandbox, saying why, a terminal shell the person opens alone being allowed. The operator mints only
  for a pod made with it. **Naming a key on a Pool changes its runners' template, as changing its image
  does: a runner made before gets no token and is no longer vouched for, so claim a new one.**
- **The token stays in the runner's memory**, and goes when its chat ends: the runner revokes it
  with GitHub. A revoke that fails, or a runner stopped first, leaves it to expire within its hour.

### Where a runner asks

**`Claim.status.mint`** holds `address` (the port as `<cluster IP>:8443`) and `operatorKey` (spelled
as a Pool's `signingKey`), only for a runner the operator vouches for on a Pool that opts in. As the
desktop pairs its runner it takes the address, and tells the runner the key it pinned when it added the
pool, whatever key the status names. A status naming another address only fails the runner's handshake. The runner keeps them in
[`mint-port.json`](plane-format.md) in its session host's folder, so it needs no way to the
Kubernetes API.

**The runner follows a roll of the signing key by itself.** The port's TLS shows the current key
alone. `POST /forge/next-key` answers, during a roll, the same next key and word from the current
key as every Pool's status holds, and a runner holding the current key checks that word as a desktop
does, then holds both till the port shows the next. It looks about hourly and before a mint. A runner paired while its desktop has already moved to the next key has its mints go to the desktop until the port shows that key. **A
runner that was stopped through the whole roll, past the old key's removal, does not follow it:** its
mints are refused and go to its desktop until a new runner is claimed.

## Tests

- `cargo test` in `operator/`, with `helm` 4 on PATH: what a Claim asks of upstream, that
  every field the operator writes is one the pinned upstream declares, what doctor finds on
  clusters the tests describe (and that it only reads), that the operator acts only on Claims
  admitted under its policy, the install manifests, and the chart, rendered and linted by Helm
  itself: the plain manifests are what it renders, its image is named only by digest, and no
  value but `forge.keySecretNames` changes what the operator may do. The definitions are generated from the Rust types and
  the plain manifests from the chart; regenerate them, in this order, with
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_install_manifests` and
  `PURLIS_OPERATOR_BLESS=1 cargo test --test the_chart`.
- [`operator/e2e/kind.sh`](../operator/e2e/kind.sh) `<runner image> <operator image>`: a kind
  cluster of Kubernetes 1.36 with a registry beside it, so images are named by digest; doctor,
  which must name upstream agent-sandbox as missing; the recorded controller digest checked
  against the registry; the pinned upstream, its controller by that digest, after which doctor
  must find nothing missing; purlis's definitions, the policy and the operator installed by the
  chart in a namespace that enforces the restricted Pod Security Standard; `kubectl diff`
  showing the plain manifests would change nothing the chart made; then
  `operator/tests/on_a_kind_cluster.rs`, which checks only what the cluster shows; then the
  desktop's side, `crates/purlis-core/tests/it/a_chat_runs_on_a_cluster_runner_through_kubectl_exec.rs`:
  a Claim, the operator's statement checked against the Pool's key, a pairing over
  `kubectl exec` that must agree with it (and one reaching another runner, refused), then a
  chat on the runner through the link; then `helm uninstall`, which leaves the definitions and
  the policy. It removes everything it started. `.github/workflows/operator.yml` runs it, with
  a runner image built from the same commit, on every pull request that changes `operator/`,
  `runner-image/` or the desktop's side of a cluster runner.
- The statement both sides read is one worked example in
  [`fixtures/runner-link-key/`](../fixtures/runner-link-key/), which the operator's tests sign
  and the desktop's tests verify, byte for byte; so is a key roll's word, and the next key's
  signature over the statement.

## Moving the upstream pin

Upstream releases weekly. To move to another release: vendor its
`sandbox-with-extensions.yaml` under `operator/vendor/agent-sandbox/<version>/` in place of the
old one, with the old one's `kustomization.yaml` moved beside it; set `VERSION` in
`operator/src/upstream.rs`, and the path and tag in `operator/e2e/kind.sh`; take the
controller image's digest for the new tag from Kubernetes' image promoter
([`registry.k8s.io/images/k8s-staging-agent-sandbox/images.yaml`](https://github.com/kubernetes/k8s.io/blob/main/registry.k8s.io/images/k8s-staging-agent-sandbox/images.yaml))
and set it in the kustomization (`newTag` and `digest`) and as `CONTROLLER_DIGEST` in
`upstream.rs`; then run the tests. `upstream_is_installed_by_digest` fails while the
kustomization, the vendored manifest and `upstream.rs` disagree on the release or the digest,
and the kind run fails when the registry holds another digest under the tag.
`only_what_the_pinned_upstream_defines_is_written` fails when a field the operator writes is
gone or renamed, before a cluster does.
