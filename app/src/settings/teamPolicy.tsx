// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type SandboxState } from "../bindings";
import { inTheWindow } from "../personaHostsAllow";
import { sandboxCommandReturned } from "../sandboxAsked";
import { inertly } from "../sandboxBlocks";
import type { RowIds } from "./components";
import type { LiveSetting } from "./groups";
import { YOUR_LEVEL } from "./yourLevel";

/**
 * **The team's say over Your level** (#2071, PR C), beside the switch: why nobody goes below
 * the full sandbox where the project's `policy = "never"` (or an administrator's policy) holds
 * it, and the project's sandbox off with how you answered it, and the other answer.
 */
export function TeamPolicy({ plane, ids }: { plane: PlaneId; ids: RowIds }) {
  const [state, setState] = useState<SandboxState>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let live = true;
    void commands
      .sandboxState(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setState(answer.data);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane]);
  const off = state?.team_off ?? null;
  const answer = (accept: boolean) => {
    if (off === null || busy) return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .answerTeamSandboxOff(plane, accept)
      .then((done) => {
        if (done.status !== "ok") setSaid(done.error);
        else {
          setState(done.data);
          sandboxCommandReturned();
        }
      })
      .catch((err: unknown) => setSaid(`purlis could not keep your answer: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  const held = state?.level_held ?? null;
  return (
    <div
      id={ids.id}
      role="group"
      className="ui-setting-status"
      aria-labelledby={ids.labelledBy}
      aria-describedby={ids.describedBy}
    >
      {state === undefined ? (
        <p>Reading the project's policy…</p>
      ) : held !== null ? (
        <p>{held}</p>
      ) : off === null ? (
        <p>Each person chooses their own level on their machine.</p>
      ) : (
        <>
          <p>
            {inertly(off.said)}{" "}
            {off.answer === true
              ? "You accepted it: your chats here run with no sandbox unless you choose a level above."
              : off.answer === false
                ? "You kept it on: your chats here keep your own level."
                : "You have not answered yet: your chats here keep your own level."}
          </p>
          <div className="ui-button-row">
            {off.answer !== true && (
              <button
                type="button"
                className="ui-button"
                disabled={busy}
                onClick={() => answer(true)}
              >
                Accept
              </button>
            )}
            {off.answer !== false && (
              <button
                type="button"
                className="ui-button"
                disabled={busy}
                onClick={() => answer(false)}
              >
                Keep it on for me
              </button>
            )}
          </div>
        </>
      )}
      {said !== undefined && <p>{said}</p>}
    </div>
  );
}

/** The row of the team's policy, first under Your level. None where this page cannot answer. */
export function teamPolicyRow(plane: PlaneId): LiveSetting[] {
  if (!inTheWindow(commands, "answerTeamSandboxOff")) return [];
  return [
    {
      id: `${YOUR_LEVEL}.team`,
      label: "The project's policy",
      // The answer is this machine's, kept outside the project.
      writes: "this machine",
      help: "How far the project lets you turn the sandbox down, and its sandbox off, which reaches you only once you accept it.",
      useControl: () => ({
        grouped: true,
        control: (ids) => <TeamPolicy plane={plane} ids={ids} />,
      }),
    },
  ];
}
