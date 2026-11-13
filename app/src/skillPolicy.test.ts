// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** **What an org's skill controls say on a row** (#6031): the mark in a word or two beside the
 *  core's sentence, which the window never rewrites. */
import { describe, expect, it } from "vitest";
import { daySaid, lineOf, refusedMark, refusedSaid, sourceMark } from "./skillPolicy";

const DAY = 24 * 60 * 60;
const NOW = Date.parse("2026-11-13T09:00:00Z") / 1000;
const origin = { kind: "extension" as const, id: "acme.registry", label: "acme.registry" };

describe("a refusal's mark", () => {
  it("names the list that refuses, and counts the days a young one waits", () => {
    expect(refusedMark({ kind: "blocked", why: "x", until: null }, NOW)).toBe("Blocked");
    expect(refusedMark({ kind: "not-allowed", why: "x", until: null }, NOW)).toBe(
      "Not on the allow list",
    );
    expect(refusedMark({ kind: "too-young", why: "x", until: NOW + 2 * DAY + 60 }, NOW)).toBe(
      "Too young, 3 days to go",
    );
    expect(refusedMark({ kind: "too-young", why: "x", until: NOW + 60 }, NOW)).toBe(
      "Too young, 1 day to go",
    );
    expect(refusedMark({ kind: "too-young", why: "x", until: null }, NOW)).toBe("Too young");
  });

  it("says the core's sentence, and the day a young one can be taken", () => {
    const until = Date.parse("2026-11-19T12:00:00Z") / 1000;
    expect(daySaid(until)).toBe("Thursday 19 November");
    expect(refusedSaid({ kind: "too-young", why: "it can be installed in 6 days", until })).toBe(
      "it can be installed in 6 days: it can be taken on Thursday 19 November.",
    );
    expect(lineOf("this project's [skill_policy] could not be read")).toBe(
      "This project's [skill_policy] could not be read",
    );
    expect(lineOf("yolo from x is on this project's block list")).toBe(
      "yolo from x is on this project's block list",
    );
  });
});

describe("a source's mark", () => {
  it("is Blocked, Only <skills> allowed or Not on the allow list, and none where all is allowed", () => {
    const of = (over: object) => ({ origin, kind: null, refused: null, only: [], ...over });
    expect(sourceMark(of({}))).toBeUndefined();
    expect(sourceMark(of({ kind: "blocked", refused: "it is blocked" }))).toBe("Blocked");
    expect(
      sourceMark(of({ kind: "not-allowed", refused: "it has only pdf", only: ["pdf", "docx"] })),
    ).toBe("Only pdf, docx allowed");
    expect(sourceMark(of({ kind: "not-allowed", refused: "it is not listed" }))).toBe(
      "Not on the allow list",
    );
  });
});
