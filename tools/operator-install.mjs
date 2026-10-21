// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The cluster operator's install, pinned to one image by digest** (#2281).
//
//   node tools/operator-install.mjs <out dir> <image>@sha256:<64 hex>
//
// The checked-in install names no real image. `operator/deploy/operator.yaml` names a
// placeholder digest no registry holds (`MANIFEST_PLACEHOLDER`), and the chart's default digest
// is empty (`CHART_DIGEST_PLACEHOLDER`), which the chart refuses. This writes, into `<out dir>`,
// the three plain manifests and the chart of this checkout, with each placeholder replaced by
// the image: so what is installed names the image built from the same commit as the manifests.
// `operator-image.yml` records it for every image it publishes, and `operator/e2e/kind.sh`
// installs it, so both replace the same placeholders the same way. Fails closed: an image not
// named by a whole digest, or a placeholder missing or written twice, writes nothing.
import { cpSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** The one line of `operator/deploy/operator.yaml` that names its image. */
export const MANIFEST_PLACEHOLDER = `          image: ghcr.io/purlis/purlis-cluster-operator@sha256:${"0".repeat(64)}`;
/** The chart's default repository and digest, in `values.yaml`. */
export const CHART_REPOSITORY_PLACEHOLDER = "  repository: ghcr.io/purlis/purlis-cluster-operator";
export const CHART_DIGEST_PLACEHOLDER = '  digest: ""';

const CHART = join("operator", "charts", "purlis-cluster-operator");

/** `image` split into its repository and digest, or a refusal in words. */
function byDigest(image) {
  const match = /^([a-z0-9][a-z0-9.:_/-]*)@(sha256:[0-9a-f]{64})$/.exec(image);
  if (!match || match[1].endsWith("/")) {
    throw new Error(`${JSON.stringify(image)} is not an image named by a whole sha256 digest`);
  }
  return { repository: match[1], digest: match[2] };
}

/** `text` with its one line `placeholder` replaced by `line`. */
function replaceOnce(text, placeholder, line, what) {
  const lines = text.split("\n");
  const at = lines.flatMap((l, i) => (l === placeholder ? [i] : []));
  if (at.length !== 1) {
    throw new Error(`${what} holds its placeholder ${at.length} times, not once: ${placeholder.trim()}`);
  }
  lines[at[0]] = line;
  return lines.join("\n");
}

/** `operator.yaml`'s text, naming `image`. */
export function pinManifest(text, image) {
  byDigest(image);
  return replaceOnce(text, MANIFEST_PLACEHOLDER, `          image: ${image}`, "operator.yaml");
}

/** The chart's `values.yaml` text, its default image `image`'s repository and digest. */
export function pinChartValues(text, image) {
  const { repository, digest } = byDigest(image);
  const named = replaceOnce(
    text,
    CHART_REPOSITORY_PLACEHOLDER,
    `  repository: ${repository}`,
    "values.yaml",
  );
  return replaceOnce(named, CHART_DIGEST_PLACEHOLDER, `  digest: "${digest}"`, "values.yaml");
}

/** Writes the install of the checkout at `repo`, pinned to `image`, into `out`. */
export function pinnedInstall(repo, out, image) {
  const deploy = join(repo, "operator", "deploy");
  const manifest = pinManifest(readFileSync(join(deploy, "operator.yaml"), "utf8"), image);
  const valuesPath = join(repo, CHART, "values.yaml");
  const values = pinChartValues(readFileSync(valuesPath, "utf8"), image);
  // Everything is read and checked before anything is written.
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "operator.yaml"), manifest);
  for (const plain of ["crds.yaml", "policy.yaml"]) {
    writeFileSync(join(out, plain), readFileSync(join(deploy, plain)));
  }
  const chart = join(out, "charts", "purlis-cluster-operator");
  cpSync(join(repo, CHART), chart, { recursive: true });
  writeFileSync(join(chart, "values.yaml"), values);
}

if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  const [out, image] = process.argv.slice(2);
  if (!out || !image) {
    console.error("usage: node tools/operator-install.mjs <out dir> <image>@sha256:<digest>");
    process.exit(2);
  }
  try {
    pinnedInstall(new URL("..", import.meta.url).pathname, out, image);
  } catch (why) {
    console.error(`purlis: the operator's install was not pinned: ${why.message}`);
    process.exit(1);
  }
}
