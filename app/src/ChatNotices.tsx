// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useContext, useMemo, useState } from "react";
import {
  commands,
  type ChatBlocked,
  type OpenChat,
  type PlaneId,
  type TheirAgentsMd,
} from "./bindings";
import { CHOOSE_EDITOR, NO_EDITOR, yourEditor } from "./yourEditor";
import { Notice, NoticeOf } from "./Notice";
import { SandboxBlockNotice } from "./SandboxBlockNotice";
import { PolicyRequiresNotice } from "./SandboxChanged";
import { VaultRefusedNotice } from "./VaultRefusedNotice";
import { McpFailedNotice } from "./McpFailedNotice";
import { InputRefusedNotice } from "./InputRefusedNotice";
import { SkillsMissingNotice } from "./SkillsMissingNotice";
import { DraftReadyNotice } from "./DraftReadyNotice";
import { HostLostNotice } from "./HostLostNotice";
import { MoveNotice } from "./MoveNotice";
import { TaskQuietNotice } from "./TaskQuietNotice";
import { PersonaGrantsNotice } from "./PersonaGrantsNotice";
import { DispatchGrantNotice } from "./DispatchGrantNotice";
import { SigningRefusalNotice } from "./SigningRefusal";
import { TasksSharingNotice } from "./TasksSharingNotice";
import { BrowserAskNotice } from "./BrowserAskNotice";
import { BrowserLoginNotice } from "./BrowserLoginNotice";
import { PoolAskNotice } from "./PoolAskNotice";
import { SecretAskPaneNotice } from "./SecretAskPaneNotice";
import { WorkspaceRemovalNotice } from "./WorkspaceRemovalNotice";
import { PausedNotice } from "./PausedNotice";
import { MergeHoldNotice } from "./MergeHoldNotice";
import { ProposedChangeNotice } from "./ProposedChangeNotice";
import { asksOf, isAnAsk, noticeKey } from "./sandboxBlocks";
import { TaskPromptNotice } from "./TaskPromptNotice";
import { SkillsChangedNotice } from "./SkillsChangedNotice";
import { onItsPane } from "./inboxRules";
import type { Shown } from "./bindings";
import { SecretFillNotice } from "./SecretFillNotice";
import { type HandedFrom } from "./handedFrom";
import { stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { askSettingsLink } from "./settings/links";
import { ChatOffers } from "./ViewOpen";

/**
 * **What a chat's pane says above its terminal** (#1035, split out of `PlaneView.tsx`): its
 * restart, a by-hand start, the notes a start left, and the asks and blocks that are this
 * chat's to answer (`ChatNotices`).
 */

/**
 * What a chat's pane says of its restart (#1342, #1428), whoever asked for it. One line at a
 * time: why the last one failed, else why it has not happened yet, else that it is under way,
 * else that it waits for the person.
 */
/**
 * What the person answers a restart's Notice with: `now` asks for it (it happens when the turn
 * has ended), `mid-turn` restarts a chat that is owed one at once, ending the turn it is in, and
 * `dismiss` puts the Notice away.
 */
export type RestartAct = "now" | "mid-turn" | "dismiss";

/**
 * What an owed restart says while the chat is mid-turn (qw351): an Allow reaches a chat only
 * through the sandbox it starts with, and purlis never ends a turn by itself. Without this the
 * person pressed Allow and saw nothing happen for as long as the turn ran.
 */
export const RESTART_WAITS_FOR_THE_TURN =
  "What you allowed reaches this chat when it restarts on the same conversation. It restarts by itself once its turn ends. Restart now ends the turn it is in.";

export type RestartSaid = {
  /** The window owes it a restart: one was asked for, or a grant needs it, and it is not made. */
  owed?: boolean;
  /** Its harness reports no state, so the person restarts it when ready. */
  byHand?: boolean;
  /** It is mid-turn, so the restart waits for the turn to end: said, with Restart now. */
  midTurn?: boolean;
  /** The restart is under way. */
  running?: boolean;
  /** Why it is not restarted yet, in the core's sentence: it waits on a permission prompt. */
  notYet?: string;
  /** Why the last restart did not happen. */
  trouble?: string;
};

/**
 * **This chat is restarting** (#1428, S5): said on its pane while the core starts its new run,
 * in place of the line that asks the person to restart it by hand. Put away, it stays away for
 * this restart: it is drawn only while one is under way.
 */
export function RestartingLine({ session }: { session: number }) {
  const [away, setAway] = useState(false);
  if (away) return null;
  return (
    <Notice
      cause={`restart-running:${session}`}
      at="pane"
      label="Restart"
      onDismiss={() => setAway(true)}
    >
      Restarting this chat…
    </Notice>
  );
}

/** What the core says when a handoff has opened a chat — `handoff::Arrived` in the app. */
export type Arrived = {
  plane: string;
  session: number;
  name: string;
  /** The workspace whose strip it is filed on, or none for one at the project's root. */
  workspace: string | null;
  persona: string | null;
  /** The harness it runs, for its default name when it adopted no persona. */
  harness?: string | null;
  /** The task name the handoff gave it, which its tab says instead (charter-app#258). */
  label?: string | null;
  /** The chat that started it, by name, and that chat's workspace; and, where the core says
   *  (#1447), whether it arrives with a tab, and whether it is a task, which owes that chat a
   *  report. */
  from?: (HandedFrom & { tab?: boolean; task?: boolean }) | null;
  /** **Whether it can't tell the person it needs them** (#4759): the core's answer from its
   *  harness's card, or `true` for a program with no adapter. A task like this is announced
   *  like a handoff. */
  cant_report_asks?: boolean;
  /** **Whether it is announced** (#4759): a handoff, or a task that can't report asks, that the
   *  person did not start. The core's answer, the one its row's "new" mark starts from. */
  announced?: boolean;
};

/** A harness started by hand in a shell tab, as its banner needs it (ADR 0062). */
export type ByHandNote = { harness: string; cwd: string | null };

/**
 * **What a shell tab says when a harness is started by hand in it** (ADR 0062): that it runs
 * outside charter's session tracking, and a way to open it as a chat instead.
 *
 * In the pane's own top-left corner, over the terminal and taking no row — the property the
 * gauge keeps, for the operator's reason (a pane must not change height when charter has
 * something to say). A `status`, not an `alert`: nothing is wrong and nothing is waiting on
 * the operator, and the harness is already running whatever they press.
 */
export function ByHandBanner({
  note,
  onAnswer,
}: {
  note: ByHandNote;
  onAnswer: (open: boolean) => void;
}) {
  return (
    <Notice
      cause={`by-hand:${note.harness}`}
      at="pane"
      label={`${note.harness} started by hand`}
      fixes={[{ label: "Open as chat", onPress: () => onAnswer(true) }]}
      onDismiss={() => onAnswer(false)}
    >
      {note.harness} runs outside purlis&apos;s session tracking here.
    </Notice>
  );
}

/** What a chat's start found to say (ADR 0085, V35): its lines, and the branches whose
 *  `AGENTS.md` they name as the operator's and hidden by charter's line (NO-4). */
export type StartNotes = { notes: readonly string[]; agentsMd: readonly TheirAgentsMd[] };

/** `held` with `note` said on chat `session`'s tab after its start's own lines, once (#2013):
 *  a line purlis has to say while the chat runs, such as its browsers ending. */
export function withNote(
  held: Record<number, StartNotes>,
  session: number,
  note: string,
): Record<number, StartNotes> {
  const was = held[session] ?? { notes: [], agentsMd: [] };
  if (was.notes.includes(note)) return held;
  return { ...held, [session]: { ...was, notes: [...was.notes, note] } };
}

/** A branch in a sentence or a button: the repo for its own folder, `repo/piece` for a piece.
 *  Every file one start names is in the same repository, so this tells them apart. */
export const branchName = (at: TheirAgentsMd) =>
  at.piece === null ? at.repo : `${at.repo}/${at.piece}`;
/** A branch's identity: what the moved set is kept by. */
export const branchKey = (at: TheirAgentsMd) => JSON.stringify([at.workspace, at.repo, at.piece]);

/**
 * **What a chat's start found to say** (ADR 0085, V35): why its `AGENTS.md` was not written,
 * or an `AGENTS.md` of the operator's that charter's exclude line hides. One line each, in the
 * pane's corner beside the by-hand banner and drawn the same way: a `status`, because nothing is
 * waiting on the operator and the chat is already running.
 *
 * **The operator's hidden file has its way out here** (NO-4, V91o): **Open file** hands it to
 * their editor, and **Move aside…** renames it to `AGENTS.aside.md` so `git status` shows it
 * again. It is the operator's file, so Move aside… asks first, in this same line, and the core
 * moves nothing until the second press (never over another file, and never charter's own).
 */
export function StartNotice({
  plane,
  found,
  onDismiss,
}: {
  plane: PlaneId;
  found: StartNotes;
  onDismiss: () => void;
}) {
  /** The branch whose file Move aside… is asking about. */
  const [asking, setAsking] = useState<TheirAgentsMd>();
  /** What the actions answered: where a file went, or the core's refusal. */
  const [said, setSaid] = useState<string>();
  /** The branches whose file has been moved aside, by {@link branchKey}: nothing more to do. */
  const [moved, setMoved] = useState<readonly string[]>([]);
  const left = found.agentsMd.filter((at) => !moved.includes(branchKey(at)));
  const one = left.length === 1;

  const openFile = (at: TheirAgentsMd) => {
    const editor = yourEditor();
    if (editor === undefined) {
      setSaid(NO_EDITOR);
      return;
    }
    void commands
      .openTheirAgentsMd(plane, at.workspace, at.repo, at.piece, editor)
      .then((done) => setSaid(done.status === "error" ? done.error : undefined))
      .catch((err: unknown) => setSaid(String(err)));
  };
  const moveAside = (at: TheirAgentsMd) => {
    setAsking(undefined);
    void commands
      .moveTheirAgentsMdAside(plane, at.workspace, at.repo, at.piece)
      .then((done) => {
        if (done.status === "error") {
          setSaid(done.error);
          return;
        }
        setMoved((was) => [...was, branchKey(at)]);
        setSaid(
          `The AGENTS.md of ${branchName(at)} was moved to ${done.data}: git status shows it again.`,
        );
      })
      .catch((err: unknown) => setSaid(String(err)));
  };

  if (asking !== undefined)
    return (
      <Notice
        cause="start-found"
        at="pane"
        label="What this chat's start found"
        fixes={[
          { label: "Move aside", onPress: () => moveAside(asking) },
          { label: "Keep it", onPress: () => setAsking(undefined) },
        ]}
      >
        Move the AGENTS.md of {branchName(asking)} aside? purlis renames it to AGENTS.aside.md (or
        the next free AGENTS.aside-N.md), never over a file, and git status shows it again.
      </Notice>
    );

  const fixes = left.flatMap((at) => [
    { label: one ? "Open file" : `Open ${branchName(at)}`, onPress: () => openFile(at) },
    {
      label: one ? "Move aside…" : `Move ${branchName(at)} aside…`,
      onPress: () => {
        setSaid(undefined);
        setAsking(at);
      },
    },
  ]);
  const [first, ...rest] = fixes;
  return (
    <Notice
      cause="start-found"
      at="pane"
      label="What this chat's start found"
      fixes={first === undefined ? undefined : [first, ...rest]}
      // With no editor chosen, Open file's answer goes where one is chosen (#1244, SE-22).
      link={
        said === NO_EDITOR
          ? { label: "Choose your editor", onPress: () => askSettingsLink(plane, CHOOSE_EDITOR) }
          : undefined
      }
      onDismiss={onDismiss}
    >
      {/* Once every named file is moved aside, the lines about it are no longer true. */}
      {found.agentsMd.length > 0 && left.length === 0
        ? said
        : [found.notes.join(" "), said].filter(Boolean).join(" ")}
    </Notice>
  );
}

/**
 * **What purlis has to say of one chat, on a pane** (#1481).
 *
 * **The order is who is waiting on whom**: first the Notice that waits for the person's answer
 * before anything starts (a dispatch no grant covers), then what purlis found or refused, in
 * the order they arrived here. The thing to press is the first thing read, and the first Tab
 * stop.
 *
 * **One per chat, and never one chat's state under another** (#1486): a pane's chat changes
 * when its tab is switched, so this is drawn by its chat (`key`), and what a Notice was told
 * or had answered goes with the chat it was about.
 */
export function ChatNotices({
  plane,
  session,
  byHand,
  onByHand,
  startNotes,
  onDismissStartNote,
  blocks,
  onDismissBlock,
  onRestart,
  onRestarted,
  onAllowed,
  restartSaid,
  onRestartAnswer,
  asks,
  waiting,
  onShowInbox,
  onAnswer,
  pane,
}: {
  plane: PlaneId;
  session: number;
  /** The pane whose band this is: an offer's Open beside goes beside it (#4769). */
  pane?: number;
  /** A harness started by hand in this shell tab, while its banner is up (ADR 0062). */
  byHand?: ByHandNote;
  onByHand: (open: boolean) => void;
  /** What this chat's start found to say, while it is up (ADR 0085). */
  startNotes?: StartNotes;
  onDismissStartNote: () => void;
  /** Asks for this chat's restart on its conversation, in this pane (#1362: Restart now). */
  onRestart: () => void;
  /** What this chat's sandbox blocked, newest last, while any is up (#1338). */
  blocks?: readonly ChatBlocked[];
  onDismissBlock: (block: ChatBlocked) => void;
  /** This chat started again in its place, without the sandbox, from a block's Notice (#1342). */
  onRestarted: (chat: OpenChat) => void;
  /** Something was allowed for this chat: it is owed a restart once its turn has ended. */
  onAllowed: () => void;
  /** What this pane says of this chat's restart (#1342, #1428), while there is something to say. */
  restartSaid?: RestartSaid;
  onRestartAnswer: (act: RestartAct) => void;
  /** The permission prompts this project's chats hold open on their hooks. */
  asks: readonly Shown[];
  /** This project's asks, as the registry derived them last; nothing before the first read. */
  waiting?: readonly Shown[];
  /** Opens this project's Inbox (a vault's refusal names it). */
  onShowInbox?: () => void;
  /** An answer was pressed on one of this chat's Notices: its words, and the ask it answered. */
  onAnswer?: (words: string, ask: Shown) => void;
}) {
  const running = useChatsSelect(
    useChatsHere(),
    (states) => stateOf(states, session) === "running",
  );
  /**
   * **The chat's asks its pane draws, from the registry** (#1695, spec #1688): the Inbox's own
   * list, so an answer in either place clears both, every one of them, the longest waiting
   * first (#4754). A permission is drawn only for a chat off screen (`TaskPromptNotice`); on
   * screen its prompt is in the pane itself, so it takes no place here.
   */
  const offScreen = useContext(NoticeOf) !== null;
  const paneAsks = useMemo(
    () =>
      (waiting ?? []).filter(
        (ask) => (ask.source !== "permission" && ask.source !== "terminal") || offScreen,
      ),
    [offScreen, waiting],
  );
  const drawn = useMemo(() => onItsPane(paneAsks, session), [paneAsks, session]);
  /** The blocks an Allow on this pane answered: what that Allow said stands, though the
   *  registry lists the block no more. */
  const [answeredHere, setAnsweredHere] = useState<ReadonlySet<string>>(() => new Set());
  // A block that is an ask is drawn while the registry lists it among the pane's, or once this
  // pane answered it; one that asks nothing (purlis's own, policy's) is said as it comes.
  const shown = (blocks ?? []).filter((block) => {
    const its = asksOf(block, waiting ?? []);
    if (its.length === 0) return !isAnAsk(block) || answeredHere.has(noticeKey(block));
    return its.some((ask) => drawn.includes(ask)) || answeredHere.has(noticeKey(block));
  });
  const newest = shown[shown.length - 1];
  const dispatchDrawn = drawn.filter((ask) => ask.source === "dispatch");
  return (
    <>
      {/* A dispatch to another persona that no grant covers (#1437): asked once, here. */}
      <DispatchGrantNotice plane={plane} session={session} asks={dispatchDrawn} onWay={onAnswer} />
      {/* A workspace this chat asked the person to remove (qw356): only a press removes it. */}
      <WorkspaceRemovalNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A change this chat proposed to purlis's own configuration (#1975): only you apply it. */}
      <ProposedChangeNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* What this chat's browser may reach, and the sites new to the project it held back: the
          chat waits on your answer, so it is asked where you look at the chat. */}
      <BrowserAskNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A browser login this chat suggested (#4595): only your press fills it. */}
      <BrowserLoginNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A secret this chat asks purlis to fill (#5115): only your press fills it. */}
      <SecretFillNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A key this chat's run missed, or a program it runs that its persona does not declare
          (plane #2697): the command waits on your answer. */}
      <SecretAskPaneNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* This chat paused at its budget or as looping (#6015): held still until you answer. */}
      <PausedNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A merge this chat's hook held (T1-39): only your press merges, and it asks once more. */}
      <MergeHoldNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A login or a tool's run this chat asks for on a pool's runner (plane #2697). */}
      <PoolAskNotice plane={plane} session={session} asks={drawn} onWay={onAnswer} />
      {/* A skill this chat drafted (#2772): ready in the Skill studio, which alone asks for it. */}
      <DraftReadyNotice plane={plane} session={session} />
      {/* A push of this chat's the forge refused for unsigned commits (#5630): Set up signing,
          then Sign and retry. */}
      <SigningRefusalNotice plane={plane} session={session} />
      {/* Two of this chat's tasks in one folder with no branch of their own (#1511). */}
      <TasksSharingNotice plane={plane} session={session} />
      {/* A chat off screen stopped on its harness's permission prompt: said where the person is. */}
      <TaskPromptNotice
        plane={plane}
        session={session}
        asks={asks}
        registry={
          waiting === undefined
            ? undefined
            : drawn.filter((ask) => ask.source === "permission" || ask.source === "terminal")
        }
      />
      {byHand && <ByHandBanner note={byHand} onAnswer={onByHand} />}
      {startNotes && (
        <StartNotice plane={plane} found={startNotes} onDismiss={onDismissStartNote} />
      )}
      <PersonaGrantsNotice
        plane={plane}
        session={session}
        running={running}
        owed={restartSaid?.owed === true}
        onRestart={onRestart}
      />
      {/* A policy that now requires the sandbox this chat runs without (#1431): until it
          restarts, or a restart is on its way. */}
      {restartSaid?.owed !== true && (
        <PolicyRequiresNotice plane={plane} session={session} onRestart={onRestart} />
      )}
      {newest !== undefined && (
        <SandboxBlockNotice
          key={noticeKey(newest)}
          block={newest}
          asks={asksOf(newest, waiting ?? [])}
          more={shown.length - 1}
          onDismiss={() => onDismissBlock(newest)}
          onAllowed={onAllowed}
          onAnswered={() => setAnsweredHere((was) => new Set([...was, noticeKey(newest)]))}
          onRestarted={onRestarted}
          onWay={onAnswer}
        />
      )}
      <VaultRefusedNotice
        plane={plane}
        session={session}
        requestHere={dispatchDrawn.length > 0}
        onShowInbox={onShowInbox}
        asks={waiting === undefined ? undefined : drawn.filter((ask) => ask.source === "vault")}
        onWay={onAnswer}
      />
      <McpFailedNotice plane={plane} session={session} onRestart={onRestart} />
      {/* What its pane sent that did not reach it (#4734). */}
      <InputRefusedNotice plane={plane} session={session} />
      {/* A skill its persona names that it started without (#2768). */}
      <SkillsMissingNotice plane={plane} session={session} />
      <HostLostNotice plane={plane} session={session} onRestart={onRestart} />
      {/* Its move to or from a pool's runner (#2048). */}
      <MoveNotice plane={plane} session={session} />
      <TaskQuietNotice plane={plane} session={session} />
      {/* An extension's offer of its view about this chat (#4769): one quiet note. */}
      <ChatOffers session={session} pane={pane} />
      {restartSaid?.trouble !== undefined && (
        <Notice
          cause={`restart:${session}`}
          at="pane"
          tone="trouble"
          label="Restart"
          fixes={[{ label: "Restart now", onPress: () => onRestartAnswer("now") }]}
          onDismiss={() => onRestartAnswer("dismiss")}
        >
          {restartSaid.trouble}
        </Notice>
      )}
      {restartSaid?.trouble === undefined && restartSaid?.notYet !== undefined && (
        <Notice
          cause={`restart-not-yet:${session}`}
          at="pane"
          label="Restart"
          onDismiss={() => onRestartAnswer("dismiss")}
        >
          {restartSaid.notYet}
        </Notice>
      )}
      {restartSaid?.running && restartSaid.notYet === undefined && (
        <RestartingLine session={session} />
      )}
      {!restartSaid?.running && (
        <SkillsChangedNotice plane={plane} session={session} onRestart={onRestart} />
      )}
      {restartSaid?.midTurn &&
        !restartSaid.byHand &&
        !restartSaid.running &&
        restartSaid.notYet === undefined &&
        restartSaid.trouble === undefined && (
          <Notice
            cause={`restart-waits-for-the-turn:${session}`}
            at="pane"
            label="Restart"
            fixes={[{ label: "Restart now", onPress: () => onRestartAnswer("mid-turn") }]}
          >
            {RESTART_WAITS_FOR_THE_TURN}
          </Notice>
        )}
      {restartSaid?.byHand && !restartSaid.running && restartSaid.notYet === undefined && (
        <Notice
          cause={`restart-by-hand:${session}`}
          at="pane"
          label="Restart"
          fixes={[{ label: "Restart now", onPress: () => onRestartAnswer("now") }]}
        >
          What you allowed reaches this chat when it restarts on the same conversation. Its harness
          does not say when a turn ends, so restart it when you are ready.
        </Notice>
      )}
    </>
  );
}
