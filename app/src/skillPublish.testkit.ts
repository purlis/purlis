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
  local: [],
  lands: [
    { folder: "deploy", replaces: true },
    { folder: "notify", replaces: false },
  ],
  broken: ["gone"],
  body: "Published with purlis.",
  fork: null,
  grades: [
    {
      skill: "deploy",
      when: "2026-10-30",
      version: "this one",
      harness: "claude-code",
      grader: "built-in",
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

export const LOCAL: PublishReviewView = { ...REVIEW, local: ["deploy"] };

/**
 * A source whose forge says the person's account may not push to it, and who has no fork of it
 * yet: the publish makes one (#4342).
 */
export const THROUGH_A_NEW_FORK: PublishReviewView = {
  ...REVIEW,
  fork: { path: "ada/skills", made: true, readers: "public: everyone can read what is sent" },
};

/** The same, through the fork the person has already. */
export const THROUGH_THEIR_FORK: PublishReviewView = {
  ...REVIEW,
  fork: { path: "ada/skills", made: false, readers: "public: everyone can read what is sent" },
};

/** A repo of the person's at the fork's name that is not a fork of the source. */
export const FORK_TAKEN: PublishReviewView = {
  ...REVIEW,
  stops: [
    {
      place: "ada/skills",
      said: "Your account already has it, and it is not a fork of the source, so no fork can be made there. Rename or delete it on the forge, then review again",
    },
  ],
};

export const EXPORT: ExportReview = {
  skills: ["deploy", "notify"],
  behind: REVIEW.behind,
  broken: [],
  stops: [],
};
