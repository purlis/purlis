// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, expect, it } from "vitest";
import { cleanup, renderHook, waitFor } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { closeTab, noTabs, openTab, type Tabs } from "../tabs";
import { useSignOutTerminals } from "./signOutTerminals";

const PLANE = "/home/op/project";
let asked: unknown[] = [];

beforeEach(() => {
  asked = [];
  mockIPC((cmd, args) => {
    if (cmd !== "pool_profiles") return null;
    asked.push(args);
    return { kept: [], addable: [], fresh_claim: false };
  });
});

afterEach(() => {
  cleanup();
  clearMocks();
});

const nothingFiled = () => "";

it("asks the pool's profiles again when the sign-out's tab closes, and only then (#4274)", async () => {
  const open = openTab(openTab(noTabs(), 7, "shell 1"), 8, "shell 2");
  const { result, rerender } = renderHook(({ tabs }) => useSignOutTerminals(tabs, PLANE), {
    initialProps: { tabs: open as Tabs },
  });
  // One not drawn yet has not closed.
  result.current(9, "other");
  rerender({ tabs: { ...open } });
  expect(asked).toEqual([]);

  result.current(7, "builds");
  rerender({ tabs: { ...open } });
  expect(asked).toEqual([]);

  const tabOf = (session: number) =>
    open.order.find((id) => JSON.stringify(open.byId[id]).includes(`"session":${session}`));
  const other = closeTab(open, tabOf(8) as number, nothingFiled);
  rerender({ tabs: other });
  expect(asked).toEqual([]);

  const closed = closeTab(other, tabOf(7) as number, nothingFiled);
  rerender({ tabs: closed });
  await waitFor(() => expect(asked).toHaveLength(1));
  expect(asked[0]).toMatchObject({ pool: "builds", open: { plane: PLANE } });

  rerender({ tabs: { ...closed } });
  expect(asked).toHaveLength(1);
});
