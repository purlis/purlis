// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, useSyncExternalStore } from "react";
import {
  commands,
  type BrowserLimit,
  type BrowserLimitShown,
  type BrowserLimitsShown,
} from "../bindings";
import { Field } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › This machine › Browsers** (#2023, ruled 2026-10-18): the most browsers a project's
 * chats hold, the most this device runs, and how long a chat's browser may go unused. Kept in
 * this machine's store and read at the next browser call, so a browser running past a lowered
 * limit runs on. A refusal past a limit names this group.
 *
 * **One read, shared by the three rows**, and read again after each write, so what a row shows
 * is what the store holds; a refused write is said in its row, in the core's words.
 */
export function browserLimitsGroup(): SettingsGroup {
  return {
    id: BROWSERS,
    label: "Browsers",
    help: "How many browsers chats may run on this machine, and how long one may sit unused. Stop all in the title bar ends every browser at once.",
    sub: true,
    settings: [
      row(
        "per_project",
        "Browsers per project",
        "The most browsers one project's chats hold at once. A chat past it is told to close one first.",
      ),
      row(
        "on_this_device",
        "Browsers on this machine",
        "The most browsers this machine runs at once, in every project. Its default follows the machine's memory.",
      ),
      row(
        "idle_minutes",
        "Minutes before an unused browser stops",
        "A chat's browser nobody used for this long stops, and the chat is told. 0 never stops one. A browser you drive never stops this way.",
      ),
    ],
  };
}

/** The group's address. */
export const BROWSERS = "you.machine.browsers";

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; limits: BrowserLimitsShown };

let read: Read = { state: "reading" };
let refused: Partial<Record<BrowserLimit, string>> = {};
const listeners = new Set<() => void>();
/** The newest read out: an answer to an older one is dropped. */
let reading = 0;

function told() {
  for (const one of listeners) one();
}

async function reread() {
  const mine = ++reading;
  const said = await commands
    .browserLimits()
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  if (mine !== reading) return;
  read =
    said.status === "error"
      ? { state: "trouble", trouble: said.error }
      : // A core that answered nothing (an older one, or none) is said as trouble, never drawn.
        typeof said.data === "object" && said.data !== null
        ? { state: "read", limits: said.data }
        : {
            state: "trouble",
            trouble: "purlis did not say what this machine's browser limits are.",
          };
  told();
}

function subscribe(listener: () => void) {
  listeners.add(listener);
  if (listeners.size === 1) void reread();
  return () => {
    listeners.delete(listener);
  };
}

/** Writes `to` for `limit`, or its default with `null`, then reads again. */
async function write(limit: BrowserLimit, to: number | null) {
  const done = await commands
    .setBrowserLimit(limit, to)
    .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
  const rest = { ...refused };
  Reflect.deleteProperty(rest, limit);
  refused = done.status === "error" ? { ...rest, [limit]: done.error } : rest;
  await reread();
}

function useLimits() {
  return useSyncExternalStore(subscribe, () => read);
}

/** The words a value reads as: `0` idle minutes is never. */
function valueSaid(limit: BrowserLimit, value: number): string {
  return limit === "idle_minutes" && value === 0 ? "never" : String(value);
}

function row(limit: BrowserLimit, label: string, help: string): LiveSetting {
  const id = `${BROWSERS}.${limit.split("_").join("-")}`;
  return {
    id,
    label,
    help,
    writes: "this machine",
    useControl: function useBrowserLimit() {
      const now = useLimits();
      const shown: BrowserLimitShown | undefined =
        now.state === "read" ? now.limits[limit] : undefined;
      const [typed, setTyped] = useState<string | undefined>(undefined);
      // What the store holds replaces what was typed once it is read again.
      useEffect(() => setTyped(undefined), [shown?.value]);
      const commit = () => {
        if (typed === undefined || shown === undefined) return;
        const to = Number(typed.trim());
        if (!Number.isInteger(to) || to < shown.least || to > shown.most) {
          refused = {
            ...refused,
            [limit]: `Nothing was changed: that one is set between ${shown.least} and ${shown.most}.`,
          };
          setTyped(undefined);
          told();
          return;
        }
        if (to !== shown.value) void write(limit, to);
        else setTyped(undefined);
      };
      const trouble = now.state === "trouble" ? now.trouble : refused[limit];
      return {
        error: trouble === undefined ? undefined : [trouble],
        control: (ids) => (
          <Field
            kind="text"
            ids={ids}
            inputMode="numeric"
            size={5}
            disabled={shown === undefined}
            value={typed ?? (shown === undefined ? "" : String(shown.value))}
            onChange={setTyped}
            onCommit={commit}
          />
        ),
        reset:
          shown === undefined
            ? undefined
            : {
                label: `Reset to ${valueSaid(limit, shown.default)}`,
                disabled: !shown.set,
                onReset: () => void write(limit, null),
              },
      };
    },
  };
}
