// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type PlaneId, type TeamOffState } from "./bindings";
import { Notice } from "./Notice";
import { inTheWindow } from "./personaHostsAllow";
import { SETTINGS, usePlaneChanged } from "./planeChanged";
import { sandboxCommandReturned } from "./sandboxAsked";
import { inertly } from "./sandboxBlocks";
import { useWindowFocused } from "./windowFocused";

/** Whether this page may answer the project's sandbox off: the window may, a link may not. */
export const mayAnswerTeamOff = (): boolean => inTheWindow(commands, "answerTeamSandboxOff");

/** What the ask says Accept means, after the commit and its author. */
export const TEAM_OFF_MEANS =
  "Accept, and your chats here run with no sandbox from their next start: a chat can then read files on disk, plain-file vaults included. Until you answer, your chats keep their own level. Settings › Sandbox offers the other answer later.";

/**
 * **The project's sandbox off reaches each person by one ask** (#2071, the operator's ruling
 * SO-8): a committed `[sandbox] mode = "off"` turns nobody's sandbox off by itself. The window
 * asks once, naming the commit that turned it off and its author: **Accept**, or **Keep it on
 * for me** (`answer_team_sandbox_off`, the window's alone). The answer is kept on this machine
 * for that commit, where no chat writes; an off committed again later asks again.
 */
export function TeamSandboxOffNotice({
  plane,
  canAnswer = mayAnswerTeamOff(),
}: {
  plane: PlaneId;
  /** Whether this page may answer: the window may, a link may not. */
  canAnswer?: boolean;
}) {
  const [off, setOff] = useState<TeamOffState>();
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const onDisk = usePlaneChanged([plane], SETTINGS);
  const focused = useWindowFocused();
  const [again, setAgain] = useState(0);
  const [hidden, setHidden] = useState(false);

  useEffect(() => {
    let live = true;
    void commands
      .sandboxState(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setOff(answer.data?.team_off ?? undefined);
      })
      // A state that cannot be read asks nothing: until an Accept, nothing is turned off.
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [plane, onDisk, focused, again]);

  if (off === undefined || off.answer !== null) return null;

  const answer = (accept: boolean) => {
    if (busy) return;
    setBusy(true);
    setSaid(undefined);
    void commands
      .answerTeamSandboxOff(plane, off.commit, accept)
      .then((done) => {
        if (done.status !== "ok") {
          setSaid(done.error);
          setAgain((n) => n + 1);
          return;
        }
        setOff(done.data.team_off ?? undefined);
        sandboxCommandReturned();
      })
      .catch((err: unknown) => setSaid(`purlis could not keep your answer: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  const body = (
    <p>
      {inertly(off.said)} {TEAM_OFF_MEANS}
      {!canAnswer && " Answer it from purlis's own window on this machine."}
      {said !== undefined && ` ${said}`}
    </p>
  );
  // A page that may not answer only says it, and may put it away.
  if (!canAnswer)
    return hidden ? null : (
      <Notice
        cause="sandbox-team-off"
        label="Your project turned the sandbox off"
        onDismiss={() => setHidden(true)}
      >
        {body}
      </Notice>
    );
  return (
    <Notice
      cause="sandbox-team-off"
      label="Your project turned the sandbox off"
      fixes={[
        { label: "Accept", onPress: () => answer(true), busy },
        { label: "Keep it on for me", onPress: () => answer(false), busy },
      ]}
    >
      {body}
    </Notice>
  );
}
