// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useEffect, useState } from "react";
import { commands, type PlaneId, type RestartEnds } from "./bindings";
import { inertly } from "./sandboxBlocks";

/**
 * **What an Allow that restarts the chat says beforehand** (#1986, ruling Q63): that it restarts
 * at the chat's next pause between turns, and what the restart will end, as the core read it
 * from the operating system (`what_a_restart_ends`): each program its commands started that
 * listens on a port, by name and port, and how many others are running. Where purlis cannot
 * tell, it says so, and never guesses. `undefined` while it is still being read.
 *
 * A program's name is the chat's to choose, so it is drawn apart from purlis's words, inertly.
 */
export function RestartEndsSaid({ ends }: { ends: RestartEnds | null | undefined }) {
  const when = "Allowing restarts this chat on its conversation at its next pause between turns";
  if (ends === undefined) return <>{when}.</>;
  if (ends === null)
    return <>{when}. purlis cannot tell what it has running, so anything it started may stop.</>;
  const { listening, others } = ends;
  if (listening.length === 0 && others === 0)
    return <>{when}; nothing it started is running now.</>;
  const noun = (count: number) => (count === 1 ? "program" : "programs");
  const programs = (count: number) => `${count} ${noun(count)}`;
  const parts = [
    ...listening.map((one) => (
      <>
        <code className="block-allow-target">{inertly(one.program)}</code> on port {one.port}
      </>
    )),
    ...(others > 0
      ? [<>{listening.length > 0 ? `${others} other ${noun(others)}` : programs(others)} </>]
      : []),
  ];
  return (
    <>
      {when}; {listening.length > 0 && "its "}
      {parts.map((part, at) => (
        <Fragment key={at}>
          {at > 0 && (at === parts.length - 1 ? " and " : ", ")}
          {part}
        </Fragment>
      ))}
      {others > 0 ? "it started will stop." : " will stop."}
    </>
  );
}

/**
 * What restarting chat `session` would end, read once as the Ask is drawn: `undefined` while it
 * is read, `null` where purlis cannot tell (or could not ask).
 */
export function useRestartEnds(plane: PlaneId, session: number, asked: boolean) {
  const [ends, setEnds] = useState<RestartEnds | null>();
  useEffect(() => {
    if (!asked) return;
    let gone = false;
    void commands
      .whatARestartEnds(plane, session)
      .then((read) => {
        if (!gone) setEnds(read.status === "ok" ? (read.data ?? null) : null);
      })
      .catch(() => {
        if (!gone) setEnds(null);
      });
    return () => {
      gone = true;
    };
  }, [plane, session, asked]);
  return ends;
}
