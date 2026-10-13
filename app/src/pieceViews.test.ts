// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import {
  PLANE_ROOT,
  isPlaneRoot,
  pieceDiffOf,
  pieceDiffTitle,
  pieceDiffView,
  pieceFileTitle,
  pieceFileView,
  pieceFilesView,
  pieceOf,
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
