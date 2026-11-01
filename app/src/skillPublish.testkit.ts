// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** The prototype's fake core for the publish review and the export dialog (#2781). */
import type { ExportReview, PublishReviewView, SkillSource } from "./bindings";

export const GITHUB: SkillSource = {
  remote: {
    url: "https://github.com/acme/skills",
    branch: null,
    label: "https://github.com/acme/skills",
  },
  folder: null,
  vault: null,
  key: null,
  kept: "shared",
  label: "https://github.com/acme/skills",
} as SkillSource;

export const REVIEW: PublishReviewView = {
  request: "pull request",
  forge: "GitHub",
  base: "main",
  commit_short: "54fe345",
  branch: "purlis/publish/deploy-3fa91c07d2be",
  title: "Add the skill deploy and 1 it refers to",
  author: "Ada Example, the address in git config",
  files: [
    { path: "deploy/SKILL.md", bytes: 1434, runs: false, comes_along: false },
    { path: "deploy/scripts/release.sh", bytes: 820, runs: true, comes_along: false },
    { path: "notify/SKILL.md", bytes: 540, runs: false, comes_along: true },
  ],
  behind: [
    { path: "deploy/.env", why: "its name is a credential file's" },
    { path: "deploy/hosts", why: "it is a link or not a plain file" },
  ],
  stops: [],
  local: false,
  grades: [
    {
      skill: "deploy",
      when: "2026-10-30",
      version: "this one",
      harness: "claude-code",
      with_it: "90%",
      without_it: "50%",
      runs: 3,
      ended: "whole",
    },
  ],
  accepted: { base: "main", commit: "5".repeat(40), digest: "d".repeat(64) },
};

export const STOPPED: PublishReviewView = {
  ...REVIEW,
  stops: [
    {
      place: "deploy/refs/checklist.md:14",
      said: "Holds a token by its forge's prefix: ghp_***",
    },
  ],
};

export const LOCAL: PublishReviewView = { ...REVIEW, local: true };

export const EXPORT: ExportReview = {
  skills: ["deploy", "notify"],
  behind: REVIEW.behind,
  stops: [],
};
