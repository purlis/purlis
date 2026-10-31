// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Fragment, useEffect, useState } from "react";
import { atAPause, useChatsHere, useChatsSelect } from "./chatState";
import { commands, type PlaneId, type RestartEnds } from "./bindings";
import { inertly } from "./sandboxBlocks";

/**
 * **What an Allow that restarts the chat says beforehand** (#1986, ruling Q63): when it restarts
 * — now, where the chat is between turns already (`now`), else at its next pause between turns —
 * and what the restart will end, as the core read it from the operating system
 * (`what_a_restart_ends`): each program its commands started that listens on a port, by name and
 * port, and how many others are running. Where purlis cannot tell, it says so, and never
 * guesses. `undefined` while it is still being read. `chat` names the chat where the words are
 * read away from its own pane (the Inbox); on its pane it is "this chat".
 *
 * A program's name is the chat's to choose, so it is drawn apart from purlis's words, inertly.
 */
export function RestartEndsSaid({
  ends,
  chat,
  now = false,
}: {
  ends: RestartEnds | null | undefined;
  chat?: string;
  now?: boolean;
}) {
  const when = `Allowing restarts ${chat ?? "this chat"} on its conversation ${
    now ? "now, as it is between turns" : "at its next pause between turns"
  }`;
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

/**
 * **The restart's sentence for chat `session`'s Ask** ({@link RestartEndsSaid}), with what it
 * ends read once as the Ask is drawn, and whether it restarts now: the chat's harness says it is
 * between turns (`waiting`), so the restart an Allow owes is made at once. A chat waiting on its
 * own work is not (#3887): its restart waits for that work, as `restartsDue` does.
 */
export function RestartEndsLine({
  plane,
  session,
  chat,
}: {
  plane: PlaneId;
  session: number;
  chat?: string;
}) {
  const ends = useRestartEnds(plane, session, true);
  const now = useChatsSelect(useChatsHere(), (states) => atAPause(states, session));
  return <RestartEndsSaid ends={ends} chat={chat} now={now} />;
}
