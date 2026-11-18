// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  isFileView,
  outsideFileOf,
  outsideFileTitle,
  outsideFileView,
  runnerFileTitle,
  runnerFileView,
  PLANE_ROOT,
  isPlaneRoot,
  pieceDiffOf,
  pieceDiffTitle,
  pieceDiffView,
  pieceFileTitle,
  pieceFileView,
  pieceFilesView,
  pieceOf,
  isWorkspaceFiles,
  workspaceFiles,
} from "./pieceViews";

const CUT = { workspace: "alpha", repo: "svc", piece: "fix-it" };

describe("a piece's files and one of its files are views named by data", () => {
  it("names the piece's files by its workspace, repo and piece", () => {
    expect(pieceOf(pieceFilesView(CUT))).toEqual({ place: CUT });
  });

  it("names one file by the piece and its path, slashes and all", () => {
    expect(pieceOf(pieceFileView(CUT, "src/a/b.rs"))).toEqual({ place: CUT, path: "src/a/b.rs" });
  });

  it("names a file of the repo's own folder, which has no piece (#948)", () => {
    const own = { workspace: "alpha", repo: "svc", piece: null };

    expect(pieceOf(pieceFileView(own, "src/a.rs"))).toEqual({ place: own, path: "src/a.rs" });
    expect(pieceOf(pieceFilesView(own))).toEqual({ place: own });
    expect(pieceFileTitle(own, "src/a.rs")).toBe("a.rs · svc");
  });

  it("names a file of a workspace's own files by the workspace and no repo (#5375)", () => {
    const own = workspaceFiles("alpha");

    expect(isWorkspaceFiles(own)).toBe(true);
    expect(isWorkspaceFiles({ workspace: "alpha", repo: "svc", piece: null })).toBe(false);
    expect(isWorkspaceFiles(PLANE_ROOT)).toBe(false);
    expect(pieceOf(pieceFileView(own, "notes/plan.md"))).toEqual({
      place: own,
      path: "notes/plan.md",
    });
    expect(pieceFileTitle(own, "notes/plan.md")).toBe("plan.md · alpha");
    // A piece with no repo names nothing.
    expect(pieceOf({ from: null, view: "piece-file", key: "alpha//fix/notes.md" })).toBeUndefined();
  });

  it("names a file of the plane root by its path alone, in a view of its own kind (#1916)", () => {
    const view = pieceFileView(PLANE_ROOT, "docs/adr/0038.md");

    // No workspace and no repo is in its key: the plane root is neither's.
    expect(view).toEqual({ from: null, view: "plane-file", key: "docs/adr/0038.md" });
    expect(pieceOf(view)).toEqual({ place: PLANE_ROOT, path: "docs/adr/0038.md" });
    expect(pieceFileTitle(PLANE_ROOT, "docs/adr/0038.md")).toBe("0038.md · Plane root");
    expect(pieceOf({ from: null, view: "plane-file", key: "" })).toBeUndefined();
    expect(pieceOf({ from: "ext", view: "plane-file", key: "a.md" })).toBeUndefined();
  });

  it("takes no workspace's place for the plane root, and no key of names for it (#1916)", () => {
    expect(isPlaneRoot(PLANE_ROOT)).toBe(true);
    expect(isPlaneRoot({ workspace: "alpha", repo: "", piece: null })).toBe(false);
    expect(isPlaneRoot({ workspace: "", repo: "svc", piece: null })).toBe(false);
    expect(isPlaneRoot({ workspace: "", repo: "", piece: "one" })).toBe(false);
    // A branch's key with its names left empty is still nobody's view.
    expect(pieceOf({ from: null, view: "piece-file", key: "//README.md" })).toBeUndefined();
    expect(pieceOf({ from: null, view: "piece-files", key: "//" })).toBeUndefined();
  });

  it("is no piece's view for any other view", () => {
    expect(pieceOf({ from: null, view: "persona", key: "steward" })).toBeUndefined();
    expect(pieceOf({ from: "ext", view: "piece-files", key: "alpha/svc/fix-it" })).toBeUndefined();
    expect(pieceOf({ from: null, view: "piece-file", key: "alpha/svc" })).toBeUndefined();
  });

  it("names a file's comparison apart from the file itself (FM-11)", () => {
    const diff = pieceDiffView(CUT, "src/a/b.rs");

    expect(pieceDiffOf(diff)).toEqual({ place: CUT, path: "src/a/b.rs" });
    expect(pieceOf(diff)).toBeUndefined();
    expect(pieceDiffOf(pieceFileView(CUT, "src/a/b.rs"))).toBeUndefined();
    expect(
      pieceDiffOf({ from: null, view: "piece-diff", key: "alpha/svc/fix-it" }),
    ).toBeUndefined();
    expect(pieceDiffTitle(CUT, "src/a/b.rs")).toBe("What changed · b.rs · fix-it");
  });
});

describe("a file outside the project, as a view (#6216)", () => {
  it("is keyed by its chat and its absolute path, is a file tab, and is no piece's", () => {
    const view = outsideFileView(7, "/etc/hosts");
    expect(view.key).toBe("7//etc/hosts");
    expect(outsideFileOf(view)).toEqual({ session: 7, path: "/etc/hosts" });
    expect(outsideFileOf(outsideFileView(null, "/etc/hosts"))).toEqual({
      session: null,
      path: "/etc/hosts",
    });
    expect(isFileView(view)).toBe(true);
    expect(pieceOf(view)).toBeUndefined();
    expect(outsideFileTitle("/etc/hosts")).toBe("hosts · Outside the project");
    // A key whose path is no absolute path, or whose chat is no number, is no such view.
    expect(outsideFileOf({ from: null, view: "outside-file", key: "7/etc/hosts" })).toBeUndefined();
    expect(
      outsideFileOf({ from: null, view: "outside-file", key: "x//etc/hosts" }),
    ).toBeUndefined();
  });
});

describe("a file on a runner chat's runner, as a view (#6387)", () => {
  it("is keyed by its chat, its runner and its path there, is a file tab, and is no piece's", () => {
    const view = runnerFileView(7, "docker/runner-1", "/home/purlis/projects/p/a b.rs");
    expect(outsideFileOf(view)).toEqual({
      session: 7,
      runner: "docker/runner-1",
      path: "/home/purlis/projects/p/a b.rs",
    });
    expect(outsideFileOf(runnerFileView(null, "r", "/etc/hosts"))).toEqual({
      session: null,
      runner: "r",
      path: "/etc/hosts",
    });
    expect(isFileView(view)).toBe(true);
    expect(pieceOf(view)).toBeUndefined();
    // Not the same tab as the same path on this machine, or on another runner.
    expect(view.key).not.toBe(outsideFileView(7, "/home/purlis/projects/p/a b.rs").key);
    expect(view.key).not.toBe(
      runnerFileView(7, "docker/runner-2", "/home/purlis/projects/p/a b.rs").key,
    );
  });

  it("is titled by its name and says it is in the runner, read-only", () => {
    expect(runnerFileTitle("docker/runner-1", "/w/src/main.rs")).toBe(
      "main.rs · In runner docker/runner-1, read-only",
    );
  });

  it("is no such view for a key whose runner or path is missing", () => {
    for (const key of ["7/r/etc/hosts", "7//etc/hosts", "x/r//etc/hosts", "7/r"])
      expect(outsideFileOf({ from: null, view: "runner-file", key })).toBeUndefined();
  });
});
