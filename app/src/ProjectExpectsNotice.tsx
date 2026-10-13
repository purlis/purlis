// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type ExpectedItem, type PlaneId, type ProjectExpects } from "./bindings";
import { Notice } from "./Notice";
import { inTheWindow } from "./personaHostsAllow";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { sandboxCommandReturned } from "./sandboxAsked";
import { inertly } from "./sandboxBlocks";
import { useWindowFocused } from "./windowFocused";

/** What the Allow says, here and on Settings › Sandbox. */
export const ALLOW_EXPECTS = "Allow on this machine";

/** Whether this page may answer for the person: the window's alone (`WINDOW_ONLY`). */
export const mayAnswerExpects = (): boolean => inTheWindow(commands, "allowProjectExpects");

/** "the local socket" or "the system service", as the item's line starts. */
const kind = (one: ExpectedItem) =>
  one.what === "socket" ? "The local socket" : "The system service";

/** Whether an Allow would keep `one`: it can be allowed on this machine now. */
export const allowable = (one: ExpectedItem) => one.refused === null;

/**
 * **One line for each thing a project expects**: what it is, named whole and drawn inertly (a
 * repository wrote it), what allowing it means in purlis's own words, and, where it cannot be
 * allowed on this machine now, why.
 */
export function ExpectedLines({ items }: { items: readonly ExpectedItem[] }) {
  return (
    <>
      {items.map((one) => (
        <p key={`${one.what}:${one.target}`}>
          {kind(one)} <code className="block-allow-target">{inertly(one.target)}</code>. {one.says}
          {one.refused !== null && <> It cannot be allowed here now: {one.refused}</>}
        </p>
      ))}
    </>
  );
}

/**
 * **This project expects something this machine has not allowed** (ADR 0093, ruling 2; qw354):
 * a project's committed settings may declare the local sockets and system services it needs,
 * and a declaration never grants. So the window asks once, up front, where the declaration is
 * read (when the project opens, and when its settings change on disk), before any chat needs
 * it. Nobody learns of it from a tool that failed.
 *
 * - **Allow on this machine** keeps each item listed that can be allowed, for every chat of this
 *   project on this machine (`allow_project_expects`): the window's own call, audited, never a
 *   chat's. Chats already running take it when they restart, and the Sandbox changed Notice
 *   lists them.
 * - **Not now** is remembered on this machine (`decline_project_expects`), and Settings ›
 *   Sandbox still offers the Allow. The question returns only when the project declares
 *   something new.
 * - **Locked by policy**: it says who locked it and offers nothing but "Got it".
 */
export function ProjectExpectsNotice({
  plane,
  onReview,
  canAnswer = mayAnswerExpects(),
}: {
  plane: PlaneId;
  /** Opens Settings at the project's Sandbox group. */
  onReview: () => void;
  /** Whether this page may answer: the window may, a link may not. */
  canAnswer?: boolean;
}) {
  const [expects, setExpects] = useState<ProjectExpects>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const onDisk = usePlaneChanged([plane], SETTINGS);
  // And when the window comes back: an Allow made in Settings, or in another window.
  const focused = useWindowFocused();

  useEffect(() => {
    let live = true;
    void commands
      .sandboxState(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setExpects(answer.data?.expects ?? undefined);
      })
      // A project whose state cannot be read asks nothing here: a declaration grants nothing,
      // so saying nothing is the safe way to be wrong.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused]);

  if (expects === undefined || !expects.asked || expects.items.length === 0) return null;

  const shown = expects.items.map((one) => one.target);
  const answer = (allow: boolean) => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    void (
      allow
        ? commands.allowProjectExpects(plane, shown)
        : commands.declineProjectExpects(plane, shown)
    )
      .then((done) => {
        if (done.status !== "ok") {
          setSaid(done.error);
          return;
        }
        setExpects(done.data.expects ?? undefined);
        // The chats open now keep the sandbox they started with: their Notice asks again.
        if (allow) sandboxCommandReturned();
      })
      .catch((err: unknown) => setSaid(`purlis could not keep your answer: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  const locked = expects.locked;
  const some = expects.items.some(allowable);
  const notNow = { label: "Not now", onPress: () => answer(false), busy };
  return (
    <Notice
      cause="sandbox-expects"
      label="This project expects more than this machine allows"
      fixes={
        locked !== null
          ? [
              { label: "Got it", onPress: () => answer(false), busy },
              { label: "Review the sandbox", onPress: onReview },
            ]
          : canAnswer && some
            ? [{ label: ALLOW_EXPECTS, onPress: () => answer(true), busy }, notNow]
            : canAnswer
              ? [notNow, { label: "Review the sandbox", onPress: onReview }]
              : [{ label: "Review the sandbox", onPress: onReview }]
      }
    >
      <p>
        This project's settings say its chats expect what is listed here. Not yet allowed on this
        machine, so a sandboxed chat is refused it. Anyone who can change those settings can change
        this list, so nothing on it is allowed until you allow it here.
      </p>
      <ExpectedLines items={expects.items} />
      <p>
        {locked !== null
          ? locked
          : "A chat takes it from its next start. Not now is remembered on this machine, and Settings › Sandbox still offers it."}
        {locked === null && !canAnswer && " Allow it from purlis's own window on this machine."}
        {said !== undefined && ` ${said}`}
      </p>
    </Notice>
  );
}
