// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from "vitest";
import { chatNumberIn } from "./chatNumber";

describe("the chat number a search asks for (#6213)", () => {
  it("is a whole number, bare or after #, spaces round it ignored", () => {
    expect(chatNumberIn(" 641 ")).toBe(641);
    expect(chatNumberIn("#64")).toBe(64);
    expect(chatNumberIn("64 tidy")).toBeUndefined();
  });

  it("is none past the largest number a chat is dealt, which the core never takes (#7125's review)", () => {
    expect(chatNumberIn("4294967295")).toBe(4_294_967_295);
    expect(chatNumberIn("4294967296")).toBeUndefined();
    expect(chatNumberIn("99999999999")).toBeUndefined();
  });
});
