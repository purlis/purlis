// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { commands, type ExpectedItem, type PlaneId, type ProjectExpects } from "./bindings";
import { Notice } from "./Notice";
import { inTheWindow } from "./personaHostsAllow";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { useSandboxCommands } from "./sandboxAsked";
import { inertly } from "./sandboxBlocks";
import { useWindowFocused } from "./windowFocused";
import { allowable, asDrawn, asNamed } from "./projectExpects";
import { asksMoved } from "./asks";

export { allowable, asDrawn, asNamed };

/**
 * **What a project expects and this machine has not allowed, read again** (#1888): whenever one
 * of the window's sandbox commands returns, so an Allow made on this Notice or in Settings ›
 * Sandbox shows on the other, and whenever `moves` grows (a caller's own counters, summed).
 * `drawn` is what the caller has already read, where it has: nothing is read until something
 * moves. `null` is nothing waiting. A read that fails keeps what is drawn, and says so in the
 * console.
 */
export function useProjectExpects(
  plane: PlaneId,
  drawn: ProjectExpects | null | undefined,
  moves = 0,
): [ProjectExpects | null | undefined, (now: ProjectExpects | null) => void] {
  const [expects, setExpects] = useState(drawn);
  const moved = useSandboxCommands() + moves;
  /** Where nothing has moved since `drawn` was read: no read is owed. */
  const drawnAt = useRef(drawn === undefined ? undefined : moved);
  useEffect(() => {
    if (moved === drawnAt.current) return;
    let live = true;
    const unread = (why: string) =>
      console.warn(`purlis could not read what this project expects: ${why}`);
    void commands
      .sandboxState(plane)
      .then((answer) => {
        if (!live) return;
        if (answer.status === "ok") setExpects(answer.data?.expects ?? null);
        else unread(answer.error);
      })
      .catch((err: unknown) => unread(String(err)));
    return () => {
      live = false;
    };
  }, [plane, moved]);
  return [expects, setExpects];
}

/** What the Allow says, on the consent's row in Asks and on Settings › Sandbox. */
export const ALLOW_EXPECTS = "Allow on this machine";

/** Whether this page may answer for the person: the window's alone (`WINDOW_ONLY`). */
export const mayAnswerExpects = (): boolean => inTheWindow(commands, "allowProjectExpects");

/** "the local socket" or "the system service", as the item's line starts. */
const kind = (one: ExpectedItem) =>
  one.what === "socket" ? "The local socket" : "The system service";

/**
 * **One line for each thing a project expects**: what it is, named whole and drawn inertly (a
 * repository wrote it), what allowing it means in purlis's own words, and, where it cannot be
 * allowed on this machine now, why. A socket is named as an Allow would keep it (`named`, links
 * resolved), with the path the project wrote beside it where that is another: a harmless name
 * that leads to another socket is shown for what it is.
 */
export function ExpectedLines({ items }: { items: readonly ExpectedItem[] }) {
  return (
    <>
      {items.map((one) => (
        <p key={`${one.what}:${one.target}`}>
          {kind(one)} <code className="block-allow-target">{inertly(one.named)}</code>
          {one.named !== one.target && (
            <>
              {" "}
              (the project names it{" "}
              <code className="block-allow-target">{inertly(one.target)}</code>)
            </>
          )}
          . {one.says}
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
 * - **While it can be answered, it is in Asks alone** (R-205): the sandbox consent is one ask,
 *   its row offering Allow on this machine and Not now (`asking::consents`), and this Notice
 *   draws nothing beside it.
 * - **Nothing listed can be allowed here now**: it says why, and offers Not now, which is
 *   remembered on this machine (`decline_project_expects`). The question returns only when the
 *   project declares something new.
 * - **Locked by policy**: it says who locked it and offers nothing but "Got it".
 * - **A page that is not the window** answers nothing: it says to allow it from the window.
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
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  // Read when the project's settings change on disk, and when the window comes back: an Allow
  // made in another window. An Allow in Settings, in this window, is a sandbox command (#1888).
  const onDisk = usePlaneChanged([plane], SETTINGS);
  const focused = useWindowFocused();

  /** Asked again after an Allow that was refused: what waits may stand otherwise by now. */
  const [again, setAgain] = useState(0);

  // A project whose state cannot be read asks nothing here: a declaration grants nothing, so
  // saying nothing is the safe way to be wrong.
  const [expects, setExpects] = useProjectExpects(plane, undefined, onDisk + focused + again);

  if (expects === undefined || expects === null || !expects.asked || expects.items.length === 0)
    return null;
  const locked = expects.locked;
  const some = expects.items.some(allowable);
  // The sandbox consent, while it can be answered, is in Asks alone (R-205): its row there is the
  // one place it is asked and answered, so no Notice carries it too.
  if (locked === null && some && canAnswer) return null;

  const items = expects.items;
  const decline = () => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .declineProjectExpects(plane, asNamed(items))
      .then((done) => {
        if (done.status !== "ok") {
          setSaid(done.error);
          // Refused: the list is read again, so what is drawn is what stands now.
          setAgain((n) => n + 1);
          return;
        }
        setExpects(done.data.expects ?? null);
        asksMoved(plane);
      })
      .catch((err: unknown) => setSaid(`purlis could not keep your answer: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  const notNow = { label: "Not now", onPress: decline, busy };
  return (
    <Notice
      cause="sandbox-expects"
      label="This project expects more than this machine allows"
      fixes={
        locked !== null
          ? [
              { label: "Got it", onPress: decline, busy },
              { label: "Review the sandbox", onPress: onReview },
            ]
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
