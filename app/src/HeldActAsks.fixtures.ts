// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { HeldActShown } from "./bindings";

/** A commit id of one digit. */
const sha = (digit: string) => digit.repeat(40);

/**
 * **The four held acts, as the core shows them** (`purlis_core::heldact::prepare`): the fields a
 * digest covers, which the tests and the stories draw and check by.
 */
export const TAG: HeldActShown = {
  ask: 1,
  session: 3,
  kind: "tag",
  repo: "acme/app",
  rows: [
    { label: "Repo", value: "acme/app" },
    { label: "Tag", value: "v1.4.0" },
    { label: "Commit", value: sha("2") },
    {
      label: "On the default branch",
      value: "No. Nobody merged it there, so the tag marks a commit main does not have",
    },
  ],
  body: null,
  digest: "digest-tag",
};

export const RELEASE: HeldActShown = {
  ask: 2,
  session: 3,
  kind: "release",
  repo: "acme/app",
  rows: [
    { label: "Repo", value: "acme/app" },
    { label: "Tag", value: "v1.4.0" },
    { label: "Commit", value: sha("1") },
    { label: "On the default branch", value: "Yes, it is on main" },
    { label: "Makes the tag", value: "Yes, at this commit" },
    { label: "Title", value: "One point four" },
    { label: "Pre-release", value: "No" },
  ],
  body: { label: "Notes", text: "Faster startup.\nFixes the crash on first launch." },
  digest: "digest-release",
};

export const CI_PUSH: HeldActShown = {
  ask: 3,
  session: 3,
  kind: "CI-file push",
  repo: "acme/app",
  rows: [
    { label: "Repo", value: "acme/app" },
    { label: "Branch", value: "ci-pin" },
    { label: "New tip", value: sha("9") },
    { label: "Compared with", value: `the branch's tip (${sha("1")})` },
    { label: "CI files", value: ".github/workflows/ci.yml" },
  ],
  body: {
    label: "Changed lines",
    text: [
      "--- a/.github/workflows/ci.yml",
      "+++ b/.github/workflows/ci.yml",
      "@@ -3,1 +3,1 @@",
      "-    runs-on: ubuntu-22.04",
      "+    runs-on: ubuntu-24.04",
    ].join("\n"),
  },
  digest: "digest-ci",
};

export const FORK: HeldActShown = {
  ask: 4,
  session: 3,
  kind: "fork",
  repo: "acme/app",
  rows: [
    { label: "Source", value: "acme/app" },
    { label: "Goes to", value: "your own account, aaron" },
    { label: "The host allows it", value: "Yes" },
  ],
  body: null,
  digest: "digest-fork",
};

/** Every kind, in the order Asks lists them. */
export const EVERY_KIND = [TAG, RELEASE, CI_PUSH, FORK] as const;
