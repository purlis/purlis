# The operator's install names its image by digest, 2026-10-21

The checked-in install of the cluster operator names no real image (#2237): `operator.yaml`
names a placeholder digest no registry holds, and the chart's default digest is empty, which the
chart refuses. The install a person downloads should name the image built from the same
commit, and it should move with every image, without CI writing to `main` (#2281). Items 1 to 3
are **decided in implementation, to be confirmed** (D-2281-1 to D-2281-3).

1. **One tool replaces the placeholders.** `tools/operator-install.mjs <out> <image>@sha256:<digest>`
   writes this checkout's plain manifests and chart into `<out>`. It sets `operator.yaml`'s image
   and the chart's default `image.repository` and `image.digest` to the image. It refuses an
   image not named by a whole sha256 digest, a placeholder missing or written twice, and an
   `<out>` that is not empty, and in each case writes nothing. *D-2281-1.*
2. **Where the downloadable install is.** `operator-image.yml` runs it for every image it
   publishes, and the run's `operator-image` artifact holds `install/` beside the digest: the
   manifests and the chart of that image's own commit. The checked-in files keep their
   placeholders, so CI writes nothing to `main`. A GitHub release of the operator, which would
   attach the same `install/`, waits for the maintainer's ruling to publish the package (ruled
   2026-10-14: private until then). *D-2281-2.*
3. **The kind tests install what is downloaded.** `operator/e2e/kind.sh` pins its install with
   the same tool. It installs the chart from it with no `--set` for the image, and diffs the
   pinned plain manifests against the cluster. A test holds both the kind run and the publishing
   run to the tool, and holds the placeholder to the tool alone. *D-2281-3.*
