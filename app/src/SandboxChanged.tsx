// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { commands, type OlderSandbox, type OnOlderSandbox, type PlaneId } from "./bindings";
import { stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { listen } from "./here";
import { Notice } from "./Notice";

/** The family of the Notice's causes: one for each chat's change (`sandbox-changed:<change>`). */
export const SANDBOX_CHANGED = "sandbox-changed";

/** What the core sends every window when this machine's sandbox policy changes (#1431). */
export const POLICY_CHANGED = "sandbox-policy-changed";

/**
 * **The last answer for each project**, kept beside the hook that asked (#1431): what a chat's
 * own tab reads to say that policy now requires the sandbox it runs without, with no second
 * question to the core.
 */
const lastAnswer = new Map<PlaneId, OlderSandbox | null>();
const answerWatchers = new Set<() => void>();

function keepAnswer(plane: PlaneId, older: OlderSandbox | null) {
  lastAnswer.set(plane, older);
  for (const told of answerWatchers) told();
}

function watchAnswers(told: () => void) {
  answerWatchers.add(told);
  return () => {
    answerWatchers.delete(told);
  };
}

/**
 * **Why chat `session` of `plane` is behind, where policy is the cause** (#1431): the policy's
 * sentence, naming who set it, from the project's last answer; none where it is not behind, or
 * is behind for the project's own settings.
 */
export function usePolicyBehind(plane: PlaneId, session: number): string | undefined {
  return useSyncExternalStore(watchAnswers, () => {
    const one = lastAnswer.get(plane)?.chats.find((chat) => chat.session === session);
    return one?.policy ?? undefined;
  });
}

/**
 * **The chats of this project still running under an older sandbox** (#1428), as the core says
 * (`chats_on_older_sandbox`): it compares what the project's settings decided of each chat's
 * sandbox when it started with what they decide now, so a settings file that was written and
 * decides the same answers none.
 *
 * Asked at the mount and again whenever `asked` moves: the caller makes it of what can change
 * the answer, which is the project's settings on disk, the chats that are open, and the
 * window's own sandbox commands. **And again each time the window comes back into focus**
 * (#1462): the cover for what no watcher reports, such as a sandbox file another window or a
 * hand wrote. **And each time this machine's policy changes** (#1431, `POLICY_CHANGED`). A
 * window that cannot ask says nothing.
 *
 * `undefined` until the core has answered, and `null` once it answers that no chat is behind:
 * only an answer lets a dismissal go.
 */
export function useOlderSandbox(plane: PlaneId, asked: string): OlderSandbox | null | undefined {
  const [older, setOlder] = useState<OlderSandbox | null>();
  const [focused, setFocused] = useState(0);
  useEffect(() => {
    const again = () => setFocused((n) => n + 1);
    window.addEventListener("focus", again);
    // **And when this machine's policy changes** (#1431): the core reads its file now and
    // then, and says so, so a policy that lands is told with nothing else moving.
    let gone = false;
    let stop: (() => void) | undefined;
    void listen(POLICY_CHANGED, again)
      .then((unlisten) => {
        if (gone) unlisten();
        else stop = unlisten;
      })
      .catch(() => undefined);
    return () => {
      gone = true;
      stop?.();
      window.removeEventListener("focus", again);
    };
  }, []);
  useEffect(() => {
    let gone = false;
    void commands
      .chatsOnOlderSandbox(plane)
      .then((said) => {
        if (gone || said.status !== "ok") return;
        const now = answered(said.data);
        keepAnswer(plane, now);
        setOlder(now);
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [plane, asked, focused]);
  return older;
}

/** The core's answer, or none: an older core, or a stand-in, may send something else. */
function answered(said: unknown): OlderSandbox | null {
  if (said === null || typeof said !== "object") return null;
  const { chats } = said as Partial<OlderSandbox>;
  if (!Array.isArray(chats)) return null;
  const behind = chats.filter(
    (one: Partial<OnOlderSandbox> | null) =>
      typeof one?.session === "number" && typeof one.change === "string",
  );
  return behind.length > 0 ? { chats: behind } : null;
}

/** What a dismissal of the Notice is kept by for one chat: the change that chat is behind. */
const causeOf = (one: OnOlderSandbox) => `${SANDBOX_CHANGED}:${one.change}`;

/**
 * **A sandbox setting changed while chats were running** (#1428). A chat's sandbox is compiled
 * as it starts, so those chats keep the one they have until they restart, and before this
 * Notice nothing said so. One Notice for each change, however many chats it left behind.
 *
 * **Restart them** asks for each chat's restart on its conversation (`onRestart`): the same
 * restart a chat tab's Restart chat row asks for, so a chat mid-turn restarts when its turn
 * ends. A chat that reports no state restarts at once, and the Notice says so in place of
 * promising a wait it cannot keep (docs/ui-copy.md, "Uncertainty is stated, not hidden").
 *
 * **Dismiss** puts the Notice away until the sandbox changes again. A dismissal is kept for
 * each chat by the change that chat is behind (`OnOlderSandbox.change`), so one of them
 * restarting does not bring the Notice back for the rest, and a chat that falls behind a
 * change nobody dismissed does.
 *
 * **Where this machine's policy is the cause** (#1431, `OnOlderSandbox.policy`), it says so,
 * and who set it, in the policy's own sentence: a chat running without the sandbox that policy
 * now requires is behind it whatever the project's settings say.
 *
 * No way to start a chat without the sandbox is offered here: a restart is sandboxed.
 */
export function SandboxChangedNotice({
  older,
  dismissed,
  dismiss,
  settle,
  onRestart,
}: {
  /** The core's answer: `undefined` before it has one, `null` for no chat behind. */
  older: OlderSandbox | null | undefined;
  dismissed: ReadonlySet<string>;
  dismiss: (cause: string) => void;
  settle: (family: string, present: readonly string[]) => void;
  onRestart: (sessions: readonly number[]) => void;
}) {
  const read = older !== undefined;
  const behind = useMemo(() => older?.chats ?? [], [older]);
  /** The changes the chats are behind, once each, as one word an effect can follow. */
  const present = [...new Set(behind.map(causeOf))].sort().join(" ");
  // A change that no chat is behind any more has gone: its dismissal goes with it, so the
  // same settings arrived at again later are told again. Only on the core's answer.
  useEffect(() => {
    if (read) settle(SANDBOX_CHANGED, present === "" ? [] : present.split(" "));
  }, [present, read, settle]);
  /** How many of them report no state: purlis cannot tell whether those are mid-turn. */
  const silent = useChatsSelect(
    useChatsHere(),
    (states) => behind.filter((one) => stateOf(states, one.session) === "unknown").length,
  );
  const causes = present === "" ? [] : present.split(" ");
  const news = causes.find((cause) => !dismissed.has(cause));
  if (behind.length === 0 || news === undefined) return null;
  const count = behind.length;
  const one = count === 1;
  /** The policy's sentence, where it is the cause for any of them: the same for each. */
  const policy = behind.find((chat) => chat.policy != null)?.policy ?? undefined;
  const allPolicy = policy !== undefined && behind.every((chat) => chat.policy != null);
  const dismissAll = () => {
    for (const cause of causes) dismiss(cause);
  };
  return (
    <Notice
      cause={news}
      label="Sandbox changed"
      fixes={[
        {
          label: one ? "Restart it" : "Restart them",
          onPress: () => {
            onRestart(behind.map((chat) => chat.session));
            dismissAll();
          },
        },
      ]}
      onDismiss={dismissAll}
    >
      {allPolicy ? policy : "This project's sandbox changed."}{" "}
      {!allPolicy && policy !== undefined && `${policy} `}
      {allPolicy
        ? one
          ? "1 chat runs without it until it restarts."
          : `${count} chats run without it until they restart.`
        : one
          ? "1 chat keeps the old sandbox until it restarts."
          : `${count} chats keep the old sandbox until they restart.`}{" "}
      {whatARestartDoes(count, silent)}
    </Notice>
  );
}

/**
 * What a restart from the Notice does, for `count` chats of which `silent` report no state.
 * The wait for a turn's end is promised only for a chat purlis can see the turns of.
 */
export function whatARestartDoes(count: number, silent: number): string {
  const keeps = "A restart keeps the conversation";
  if (silent === 0) return `${keeps}, and waits for a turn to end.`;
  const cannotTell = (who: string, are: string, them: string) =>
    `${who} no state, so purlis cannot tell whether ${are} mid-turn, and restarts ${them} at once.`;
  if (silent >= count)
    return count === 1
      ? `${keeps}. This chat ${cannotTell("reports", "it is", "it")}`
      : `${keeps}. These chats ${cannotTell("report", "they are", "them")}`;
  return `${keeps}, and waits for a turn to end. ${
    silent === 1
      ? cannotTell("1 of them reports", "it is", "it")
      : cannotTell(`${silent} of them report`, "they are", "them")
  }`;
}

/**
 * **On the tab of a chat running without the sandbox that policy now requires** (#1431): the
 * same words as the project's Notice, and Restart, which restarts this chat on its
 * conversation, sandboxed. Drawn from the project's last answer (`usePolicyBehind`), so it goes
 * once the chat restarts or the policy goes. Put away, it stays away for this policy.
 */
export function PolicyRequiresNotice({
  plane,
  session,
  onRestart,
}: {
  plane: PlaneId;
  session: number;
  onRestart: () => void;
}) {
  const policy = usePolicyBehind(plane, session);
  const [away, setAway] = useState<string>();
  if (policy === undefined || away === policy) return null;
  return (
    <Notice
      cause={`sandbox-policy:${session}`}
      at="pane"
      label="Sandbox required"
      fixes={[{ label: "Restart this chat", onPress: onRestart }]}
      onDismiss={() => setAway(policy)}
    >
      {policy} This chat runs without it until it restarts. A restart keeps the conversation.
    </Notice>
  );
}
