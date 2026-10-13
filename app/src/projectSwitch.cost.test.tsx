// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { appendFileSync } from "node:fs";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { switches, type Shape, type Switch } from "./projectSwitch.testkit";

await vi.hoisted(async () => {
  const { countReactsCommits } = await import("./reactCommits.testkit");
  countReactsCommits();
});

vi.mock("./SessionPane", async () => ({
  SessionPane: (await import("./countedPane.testkit")).CountedPane,
}));

afterEach(() => {
  cleanup();
  clearMocks();
});

const line = (shape: Shape, one: Switch) =>
  JSON.stringify({
    projects: shape.projects,
    chats: shape.chats,
    chatsIn: shape.chatsIn,
    ...one,
    syncMs: Math.round(one.syncMs),
  });

describe("what a project switch costs", () => {
  const matrix: Shape[] = (process.env.SWITCH_MATRIX ?? "3x20")
    .split(",")
    .map((cell) => cell.split("x").map(Number))
    .map(([projects, chats, inSecond]) => ({
      projects,
      chats,
      workspaces: 12,
      chatsIn: inSecond === undefined ? undefined : { 1: inSecond },
    }));

  for (const shape of matrix) {
    it(`${shape.projects} projects of ${shape.chats} chats ${JSON.stringify(shape.chatsIn ?? {})}`, async () => {
      const took = await switches(shape, [1, 0, 1, 0]);
      for (const one of took)
        if (process.env.SWITCH_OUT) appendFileSync(process.env.SWITCH_OUT, `${line(shape, one)}\n`);
      expect(took.length).toBe(4);
    }, 300_000);
  }
});
