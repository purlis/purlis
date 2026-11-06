// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { describe, expect, it } from "vitest";
import { imageMonospace } from "./monospace";

describe("the image's own monospace (#5402)", () => {
  it("is the family fontconfig names first for monospace", () => {
    const asked: string[][] = [];
    const family = imageMonospace((command, args) => {
      asked.push([command, ...args]);
      return "WenQuanYi Zen Hei Mono";
    });
    expect(family).toBe("WenQuanYi Zen Hei Mono");
    expect(asked).toEqual([["fc-match", "-f", "%{family[0]}", "monospace"]]);
  });

  it("is that family without the whitespace around it", () => {
    expect(imageMonospace(() => "  Liberation Mono\n")).toBe("Liberation Mono");
  });

  it("is none on a machine without fontconfig", () => {
    expect(
      imageMonospace(() => {
        throw new Error("spawnSync fc-match ENOENT");
      }),
    ).toBeUndefined();
  });

  it("is none when fontconfig names no family", () => {
    expect(imageMonospace(() => "")).toBeUndefined();
  });
});
