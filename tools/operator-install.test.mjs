// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The cluster operator's install, pinned to one image by digest (#2281): what
// `operator-image.yml` records for each image it publishes and what `operator/e2e/kind.sh`
// installs, made by the one tool from the same placeholders. Run with `node --test tools/`.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  CHART_DIGEST_PLACEHOLDER,
  MANIFEST_PLACEHOLDER,
  pinChartValues,
  pinManifest,
  pinnedInstall,
} from "./operator-install.mjs";

const repo = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(repo, path), "utf8");
const IMAGE = `ghcr.io/purlis/purlis-cluster-operator@sha256:${"a".repeat(64)}`;

test("the checked-in manifest and chart each hold their placeholder exactly once", () => {
  const manifest = read("operator/deploy/operator.yaml").split("\n");
  assert.equal(manifest.filter((l) => l === MANIFEST_PLACEHOLDER).length, 1);
  const values = read("operator/charts/purlis-cluster-operator/values.yaml").split("\n");
  assert.equal(values.filter((l) => l === CHART_DIGEST_PLACEHOLDER).length, 1);
});

test("the manifest is pinned to the image by digest, and nothing else of it moves", () => {
  const text = read("operator/deploy/operator.yaml");
  const pinned = pinManifest(text, IMAGE);
  assert.ok(pinned.split("\n").includes(`          image: ${IMAGE}`));
  assert.ok(!pinned.includes("sha256:0000000000"), "no placeholder is left");
  assert.equal(pinned.split("\n").length, text.split("\n").length);
});

test("the chart's default values name the image's repository and digest", () => {
  const text = read("operator/charts/purlis-cluster-operator/values.yaml");
  const pinned = pinChartValues(text, "localhost:5001/purlis-cluster-operator@sha256:" + "b".repeat(64));
  const lines = pinned.split("\n");
  assert.ok(lines.includes("  repository: localhost:5001/purlis-cluster-operator"));
  assert.ok(lines.includes(`  digest: "sha256:${"b".repeat(64)}"`));
  assert.ok(!lines.includes(CHART_DIGEST_PLACEHOLDER));
});

test("an image not named by a whole digest is refused", () => {
  const text = read("operator/deploy/operator.yaml");
  for (const bad of [
    "ghcr.io/purlis/purlis-cluster-operator:main",
    "ghcr.io/purlis/purlis-cluster-operator@sha256:abc",
    `ghcr.io/purlis/purlis-cluster-operator@sha256:${"A".repeat(64)}`,
    `ghcr.io/purlis/purlis cluster@sha256:${"a".repeat(64)}`,
    `ghcr.io/purlis/x"y@sha256:${"a".repeat(64)}`,
    `@sha256:${"a".repeat(64)}`,
  ]) {
    assert.throws(() => pinManifest(text, bad), /digest/, bad);
  }
});

test("a manifest whose placeholder is missing or doubled is refused", () => {
  assert.throws(() => pinManifest("kind: Deployment\n", IMAGE), /placeholder/);
  const twice = `${MANIFEST_PLACEHOLDER}\n${MANIFEST_PLACEHOLDER}\n`;
  assert.throws(() => pinManifest(twice, IMAGE), /placeholder/);
  assert.throws(() => pinChartValues("image: {}\n", IMAGE), /placeholder/);
});

test("the pinned install is the three manifests and the chart, of the same checkout", () => {
  const out = mkdtempSync(join(tmpdir(), "purlis-operator-install-"));
  pinnedInstall(repo, out, IMAGE);
  assert.deepEqual(readdirSync(out).sort(), ["charts", "crds.yaml", "operator.yaml", "policy.yaml"]);
  for (const plain of ["crds.yaml", "policy.yaml"]) {
    assert.equal(readFileSync(join(out, plain), "utf8"), read(`operator/deploy/${plain}`), plain);
  }
  assert.ok(readFileSync(join(out, "operator.yaml"), "utf8").includes(`image: ${IMAGE}`));
  const chart = join(out, "charts", "purlis-cluster-operator");
  assert.ok(readFileSync(join(chart, "values.yaml"), "utf8").includes(`digest: "sha256:${"a".repeat(64)}"`));
  assert.equal(
    readFileSync(join(chart, "templates", "deployment.yaml"), "utf8"),
    read("operator/charts/purlis-cluster-operator/templates/deployment.yaml"),
  );
});

test("the kind run and the publishing run pin their install with this tool, and nothing else does", () => {
  const kind = read("operator/e2e/kind.sh");
  assert.match(kind, /node "\$repo\/tools\/operator-install\.mjs" "\$install" "\$operator"/);
  assert.match(kind, /helm install purlis-cluster-operator "\$install\/charts\/purlis-cluster-operator"/);
  assert.match(kind, /-f "\$install\/operator\.yaml"/);
  assert.doesNotMatch(kind, /image\.digest=/, "the chart's own default names the image");
  assert.doesNotMatch(kind, /sha256:0{64}/, "the placeholder is the tool's alone");

  const workflow = read(".github/workflows/operator-image.yml");
  assert.match(workflow, /node tools\/operator-install\.mjs "\$RUNNER_TEMP\/record\/install" "\$IMAGE@\$DIGEST"/);
  assert.doesNotMatch(workflow, /sha256:0{64}/, "the placeholder is the tool's alone");
});
