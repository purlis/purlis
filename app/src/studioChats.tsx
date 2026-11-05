// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat the Skill studio started stays linked to its skill and job** (#5185;
 * `prototype/studio-chat-link`, variant A with B's Grades card, the operator's verdict of
 * 2026-11-04). The core keeps the link on the chat's own record (`reopen.json`'s
 * `chats[].studio`), so it outlives a relaunch; the window reads it with the chat
 * (`OpenChat.studio`) and when the studio starts one, and lends it here:
 *
 * - **the chat's pane foot** names the skill and the job, and opens the skill's page
 *   ({@link StudioChatChip});
 * - **the skill's page** says the chat while it runs, above its tabs, with Open chat
 *   ({@link StudioChatLines}), and lists it on Runs afterwards ({@link StudioChatsStarted}),
 *   with when it finished, and Review draft while its draft waits (#5248);
 * - **the Grades card** shows the running job in place of its button, with how long it has run
 *   and Stop it (#5248);
 * - **the draft** it wrote offers Open chat where it waits for review.
 *
 * The chats still open are lent by the window; one closed since is read from this machine's
 * record of them (`studio_chats_closed`, #5248), and listed on Runs with no Open chat, since it
 * has no tab to open.
 */
import { createContext, useContext, useEffect, useId, useMemo, useState } from "react";
import { OPEN_CHAT, REVIEW_DRAFT } from "./actions";
import {
  commands,
  type ClosedStudioChat,
  type SkillDraft,
  type SkillLevel,
  type StudioJobKind,
  type StudioLink,
} from "./bindings";
import { stateOf, useChatsHere, useChatsSelect } from "./chatState";
import { endedBy } from "./EndedBand";
import { Notice } from "./Notice";
import { chatPlace, sameLevel } from "./skillChanges";
import { useSkillDrafts, type SkillChatAsk } from "./skillDraftAsks";
import {
  AT_THE_ROOT,
  asPersona,
  inWorkspace,
  type SkillPageTab,
  type StudioPage,
  type StudioScope,
} from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { useNow } from "./stateClock";
import { Button, ButtonRow } from "./ui/button/Button";
import { List, ListRow } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { PanelSectionTitle } from "./ui/panel/Panel";

/** One open chat the studio started, with what it was started for. */
export type StudioChat = {
  session: number;
  /** What the chat is called here. */
  name: string;
  link: StudioLink;
};

/** What the project's window lends the studio and the chats' panes. */
export type StudioChatsLent = {
  /** Each open chat the studio started, in the order the window holds them. */
  chats: readonly StudioChat[];
  /** Shows chat `session`, as its row's press does. */
  show: (session: number) => void;
  /** Opens the studio at the place `link` names: its skill's page, or Drafts for a new one. */
  openSkill: (link: StudioLink) => void;
  /** Opens the draft `id` its chat wrote, on Drafts of the scope `link`'s skill is seen on. */
  openDraft: (link: StudioLink, id: string) => void;
  /** Asks to stop chat `session`, as its own Stop does. */
  stop: (session: number) => void;
};

const NONE: StudioChatsLent = {
  chats: [],
  show: () => undefined,
  openSkill: () => undefined,
  openDraft: () => undefined,
  stop: () => undefined,
};

/** **The studio's chats, lent by the project's window**: none outside one. */
export const StudioChatsHere = createContext<StudioChatsLent>(NONE);

export function useStudioChats(): StudioChatsLent {
  return useContext(StudioChatsHere);
}

/** **What each job is**, while it runs and once it is done. */
const JOBS: Record<StudioJobKind, { doing: string; did: string }> = {
  draft: { doing: "drafting a skill", did: "drafted a skill" },
  "trigger-tests": { doing: "writing trigger tests", did: "wrote trigger tests" },
  "outcome-evals": { doing: "writing outcome evals", did: "wrote outcome evals" },
  checks: { doing: "writing checks", did: "wrote checks" },
  improve: { doing: "drafting an edit from its runs", did: "drafted an edit from its runs" },
  redo: { doing: "redoing its draft", did: "redid its draft" },
};

/** The job, as a line says it: `writing outcome evals`, or `wrote outcome evals` once done. */
export function jobSaid(job: StudioJobKind, done: boolean): string {
  return done ? JOBS[job].did : JOBS[job].doing;
}

/** The job as a line of its own begins it: `Writing outcome evals`, `Wrote outcome evals`. */
export function jobLine(job: StudioJobKind, done: boolean): string {
  const said = jobSaid(job, done);
  return said.charAt(0).toUpperCase() + said.slice(1);
}

/** What the chat's chip says: `test-design · writing outcome evals`. */
export function chipSaid(link: StudioLink, done: boolean): string {
  return `${link.skill ?? "New skill"} · ${jobSaid(link.job, done)}`;
}

/** The chats started for the skill `name` at `level`, and no other's. */
export function chatsOfSkill<Chat extends { link: StudioLink }>(
  chats: readonly Chat[],
  name: string,
  level: SkillLevel,
): Chat[] {
  return chats.filter((one) => one.link.skill === name && sameLevel(one.link.level, level));
}

/** **Whether a studio chat's job is done**: its draft came (kept on its record, so a draft
 *  accepted or discarded since leaves it done), a draft it wrote waits, or its program ended. */
export function isDone(chat: StudioChat, drafts: readonly SkillDraft[], ended: boolean): boolean {
  return chat.link.done || ended || drafts.some((one) => one.chat === chat.session);
}

/** **The links a read of the drafts makes done** (#5185): each chat's whose draft came, as the
 *  core marks its record, finished `now` where it was not already (#5248). `links` itself where
 *  none changes, so nothing is drawn anew. */
export function doneByDrafts(
  links: Readonly<Record<number, StudioLink>>,
  drafts: readonly SkillDraft[],
  now = new Date(),
): Readonly<Record<number, StudioLink>> {
  const wrote = drafts.flatMap((one) =>
    one.chat !== null && links[one.chat]?.done === false ? [one.chat] : [],
  );
  if (wrote.length === 0) return links;
  return {
    ...links,
    ...Object.fromEntries(
      wrote.map((session) => [
        session,
        { ...links[session], done: true, finished: links[session].finished ?? timeKept(now) },
      ]),
    ),
  };
}

/** **The studio's scope a skill of `level` is seen on**: its workspace's, its persona's, or the
 *  project's root. */
export function scopeOfLevel(level: SkillLevel): StudioScope {
  switch (level.at) {
    case "project":
      return AT_THE_ROOT;
    case "workspace":
      return inWorkspace(level.name);
    case "persona":
      return asPersona(level.name);
  }
}

/** **Where a link opens**: the skill's Grades for its checks, its Runs for an edit or a redo,
 *  and Drafts for a new skill, which has no page until its draft is accepted. */
export function studioPlaceOf(
  link: StudioLink,
): { page: "drafts" } | { page: Extract<StudioPage, "library">; skill: string; tab: SkillPageTab } {
  if (link.skill === null) return { page: "drafts" };
  const checks =
    link.job === "checks" || link.job === "trigger-tests" || link.job === "outcome-evals";
  return { page: "library", skill: link.skill, tab: checks ? "grades" : "runs" };
}

/** **The link a studio start leaves on the chat it opens**, as the core records it: none for Try
 *  it, which is a scratch chat with no job of the studio's. */
export function linkOfAsk(ask: SkillChatAsk, now = new Date()): StudioLink | undefined {
  const job = linkOfAskAlone(ask);
  return job === undefined ? undefined : { ...job, started: timeKept(now), finished: null };
}

/** `now` as the core keeps a time: RFC 3339, in UTC, to the second. */
export function timeKept(now: Date): string {
  return now.toISOString().replace(/\.\d{3}Z$/, "Z");
}

/** {@link linkOfAsk}'s link, before it has a time. */
function linkOfAskAlone(ask: SkillChatAsk): Omit<StudioLink, "started" | "finished"> | undefined {
  switch (ask.kind) {
    case "draft":
      return { skill: null, level: ask.level, job: "draft", done: false };
    case "triggers":
      return { skill: ask.name, level: ask.level, job: "trigger-tests", done: false };
    case "evals":
      return { skill: ask.name, level: ask.level, job: "outcome-evals", done: false };
    case "checks":
      return { skill: ask.name, level: ask.level, job: "checks", done: false };
    case "improve":
      return { skill: ask.name, level: ask.level, job: "improve", done: false };
    case "redo":
      return { skill: ask.name, level: ask.level, job: "redo", done: false };
    case "try":
      return undefined;
  }
}

/**
 * **When a studio chat finished, as its row says it** (#5248; the prototype's `finished 17:42`):
 * its clock time where that was today, and its day before it, `11-04 17:42`. Nothing for a time
 * that does not read.
 */
export function finishedSaid(at: string | null, now: Date): string | undefined {
  const when = at === null ? Number.NaN : Date.parse(at);
  if (Number.isNaN(when)) return undefined;
  const then = new Date(when);
  const two = (n: number) => String(n).padStart(2, "0");
  const clock = `${two(then.getHours())}:${two(then.getMinutes())}`;
  return then.toDateString() === now.toDateString()
    ? `finished ${clock}`
    : `finished ${two(then.getMonth() + 1)}-${two(then.getDate())} ${clock}`;
}

/**
 * **How long a studio chat's job has run, as the Grades card says it** (#5248; the prototype's
 * `running 2 min`): `running under a minute`, `running 2 min`, `running 1 h 5 min`, and
 * `running` alone where its start is not known.
 */
export function runningSaid(started: string | null, now: number): string {
  const since = started === null ? Number.NaN : Date.parse(started);
  if (Number.isNaN(since)) return "running";
  const minutes = Math.max(0, Math.floor((now - since) / 60_000));
  if (minutes < 1) return "running under a minute";
  if (minutes < 60) return `running ${minutes} min`;
  return `running ${Math.floor(minutes / 60)} h ${minutes % 60} min`;
}

/**
 * **The chats the studio started that have closed here** (#5248), newest first, read again each
 * time `open` (the open studio chats' sessions) changes, since a chat that closes is kept as it
 * goes. None until read, and none where it cannot be.
 */
export function useClosedStudioChats(plane: string, open: string): readonly ClosedStudioChat[] {
  const [closed, setClosed] = useState<readonly ClosedStudioChat[]>([]);
  useEffect(() => {
    let live = true;
    void commands
      .studioChatsClosed(plane)
      .then((answer) => {
        if (live && answer.status === "ok") setClosed(answer.data);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [plane, open]);
  return closed;
}

/** Whether chat `session`'s program has ended, read off the chats' store. */
export function useEnded(session: number): boolean {
  return useChatsSelect(useChatsHere(), (states) => endedBy(stateOf(states, session)));
}

/**
 * **The chip on a studio chat's pane foot** (#5185, variant A; #5204): `test-design · writing
 * outcome evals`, beside `↳ from`, since it says where the chat came from. Its press runs the
 * chat's row, which opens the skill's page; with no such row it is its words alone. Nothing for a
 * chat the studio did not start.
 */
export function StudioChatChip({
  plane,
  session,
  onOpen,
}: {
  plane: string;
  session: number;
  /** The row a press runs (#5204): what the chat was started for. */
  onOpen?: () => void;
}) {
  const { chats } = useStudioChats();
  const chat = chats.find((one) => one.session === session);
  return chat === undefined ? null : <ChipOf plane={plane} chat={chat} onOpen={onOpen} />;
}

/** The chip of a chat the studio started: done once its draft waits or its program ended. */
function ChipOf({ plane, chat, onOpen }: { plane: string; chat: StudioChat; onOpen?: () => void }) {
  const drafts = useSkillDrafts(plane, 0);
  const ended = useEnded(chat.session);
  const done = isDone(chat, drafts, ended);
  const where = chat.link.skill === null ? "Drafts" : chat.link.skill;
  if (onOpen === undefined)
    return (
      <span className="pane-studio" data-testid="pane-studio">
        {chipSaid(chat.link, done)}
      </span>
    );
  return (
    <Button
      inline
      variant="ghost"
      className="pane-studio pane-foot-press"
      data-testid="pane-studio"
      tooltip={`Open ${where} in the Skill studio`}
      onPress={onOpen}
    >
      {chipSaid(chat.link, done)}
    </Button>
  );
}

/** One running studio chat's line above a skill's tabs. */
function StudioChatLine({
  plane,
  chat,
  onShow,
}: {
  plane: string;
  chat: StudioChat;
  onShow: () => void;
}) {
  const doing = jobSaid(chat.link.job, false);
  return (
    <Notice
      cause={`studio-chat:${chat.session}`}
      tone="news"
      label={`A chat is ${doing}`}
      fixes={[{ label: OPEN_CHAT, onPress: onShow }]}
    >
      <span data-testid={`studio-chat-running-${chat.session}`}>
        {`A chat ${chatPlace(chat.link.level, plane)} is ${doing} · running`}
      </span>
    </Notice>
  );
}

/** A running chat's line, drawn only while its program runs and no draft of it waits. */
function RunningLine({
  plane,
  chat,
  drafts,
  onShow,
}: {
  plane: string;
  chat: StudioChat;
  drafts: readonly SkillDraft[];
  onShow: () => void;
}) {
  const ended = useEnded(chat.session);
  return isDone(chat, drafts, ended) ? null : (
    <StudioChatLine plane={plane} chat={chat} onShow={onShow} />
  );
}

/**
 * **The line above a skill's tabs while a studio chat works on it** (#5185, variant A): `A chat
 * at purlis-demo is writing outcome evals · running`, with Open chat. Gone once it is done; the
 * Runs tab keeps the chat.
 */
export function StudioChatLines({
  plane,
  name,
  level,
  drafts,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  drafts: readonly SkillDraft[];
}) {
  const { chats, show } = useStudioChats();
  const mine = useMemo(() => chatsOfSkill(chats, name, level), [chats, name, level]);
  if (mine.length === 0) return null;
  return (
    <div className="skill-page-studio-chats" data-testid="skill-studio-chats-running">
      {mine.map((chat) => (
        <RunningLine
          key={chat.session}
          plane={plane}
          chat={chat}
          drafts={drafts}
          onShow={() => show(chat.session)}
        />
      ))}
    </div>
  );
}

/** One row of the chats the studio started: running or finished, and what it offers. */
function StartedRow({
  plane,
  session,
  name,
  link,
  done,
  draft,
  onShow,
  onReview,
}: {
  plane: string;
  session: number;
  name: string;
  link: StudioLink;
  done: boolean;
  /** The draft it wrote, while that waits on Drafts. */
  draft: SkillDraft | undefined;
  /** Shows the chat: absent for one that has closed, which has no tab. */
  onShow?: () => void;
  onReview: (draft: SkillDraft) => void;
}) {
  const said = jobLine(link.job, done);
  const finished = done ? finishedSaid(link.finished, new Date()) : undefined;
  const below = [
    `${name}, ${chatPlace(link.level, plane)}`,
    ...(finished === undefined ? [] : [finished]),
    ...(draft === undefined ? [] : ["its draft waits in Drafts"]),
  ].join(" · ");
  return (
    <ListRow id={String(session)} textValue={said} data-testid={`studio-chat-${session}`}>
      <RowLabel>{said}</RowLabel>
      <RowTrail>
        <Badge variant={done ? "ok" : "accent"}>{done ? "Finished" : "Running"}</Badge>
        {draft !== undefined && (
          <Button
            size="sm"
            tooltip={`Opens the draft of ${draft.name}`}
            onPress={() => onReview(draft)}
          >
            {REVIEW_DRAFT}
          </Button>
        )}
        {onShow !== undefined && (
          <Button size="sm" tooltip={`Shows ${name}`} onPress={onShow}>
            {OPEN_CHAT}
          </Button>
        )}
      </RowTrail>
      <RowBelow clamp={1}>{below}</RowBelow>
    </ListRow>
  );
}

/** An open chat's row, done once its draft waits or its program ended. */
function OpenRow({
  plane,
  chat,
  drafts,
  onShow,
  onReview,
}: {
  plane: string;
  chat: StudioChat;
  drafts: readonly SkillDraft[];
  onShow: () => void;
  onReview: (draft: SkillDraft) => void;
}) {
  const ended = useEnded(chat.session);
  return (
    <StartedRow
      plane={plane}
      session={chat.session}
      name={chat.name}
      link={chat.link}
      done={isDone(chat, drafts, ended)}
      draft={drafts.find((one) => one.chat === chat.session)}
      onShow={onShow}
      onReview={onReview}
    />
  );
}

/**
 * **"Chats the studio started"** on a skill's Runs (#5185, variant A; #5248): each chat the
 * studio started for this skill, the open ones first, running or finished, with Open chat, then
 * those closed here, newest first, each with when it finished. Review draft where its draft
 * waits. Nothing where there is none.
 */
export function StudioChatsStarted({
  plane,
  name,
  level,
  drafts,
}: {
  plane: string;
  name: string;
  level: SkillLevel;
  drafts: readonly SkillDraft[];
}) {
  const { chats, show, openDraft } = useStudioChats();
  const titleId = useId();
  const mine = useMemo(() => chatsOfSkill(chats, name, level), [chats, name, level]);
  const openNow = chats.map((one) => one.session).join(" ");
  const everClosed = useClosedStudioChats(plane, openNow);
  const closed = useMemo(
    () =>
      chatsOfSkill(everClosed, name, level).filter(
        (one) => !chats.some((open) => open.session === one.session),
      ),
    [everClosed, chats, name, level],
  );
  if (mine.length === 0 && closed.length === 0) return null;
  const draftOf = (session: number) => drafts.find((one) => one.chat === session);
  const review = (link: StudioLink) => (draft: SkillDraft) => openDraft(link, draft.id);
  return (
    <section
      className="skill-page-studio-started"
      aria-labelledby={titleId}
      data-testid="skill-studio-started"
    >
      <PanelSectionTitle id={titleId}>Chats the studio started</PanelSectionTitle>
      <List
        aria-labelledby={titleId}
        size="sm"
        onAction={(key) => {
          const session = Number(key);
          if (mine.some((one) => one.session === session)) return show(session);
          const gone = closed.find((one) => one.session === session);
          const draft = draftOf(session);
          if (gone !== undefined && draft !== undefined) openDraft(gone.link, draft.id);
        }}
      >
        {mine.map((chat) => (
          <OpenRow
            key={chat.session}
            plane={plane}
            chat={chat}
            drafts={drafts}
            onShow={() => show(chat.session)}
            onReview={review(chat.link)}
          />
        ))}
        {closed.map((chat) => (
          <StartedRow
            key={chat.session}
            plane={plane}
            session={chat.session}
            name={chat.name}
            link={chat.link}
            done
            draft={draftOf(chat.session)}
            onReview={review(chat.link)}
          />
        ))}
      </List>
    </section>
  );
}

/** **The running job of `kinds` on the skill `name`**, for the Grades card that shows it in
 *  place of its button (#5185, B's Grades card): the first such chat not done yet. */
export function useRunningJob(
  name: string,
  level: SkillLevel,
  kinds: readonly StudioJobKind[],
  drafts: readonly SkillDraft[],
): StudioChat | undefined {
  const { chats } = useStudioChats();
  const states = useChatsHere();
  const ofKinds = chatsOfSkill(chats, name, level).filter((one) => kinds.includes(one.link.job));
  // The first not done, by its session: a number the store's select compares as it is.
  const running = useChatsSelect(
    states,
    (now) =>
      ofKinds.find((one) => !isDone(one, drafts, endedBy(stateOf(now, one.session))))?.session,
  );
  return ofKinds.find((one) => one.session === running);
}

/** **The Grades card's running job** (#5185; #5248, B's Grades card): what the chat does, where
 *  and for how long, with Open chat and Stop it. */
export function RunningJob({ plane, chat }: { plane: string; chat: StudioChat }) {
  const { show, stop } = useStudioChats();
  const now = useNow();
  const doing = jobSaid(chat.link.job, false);
  return (
    <div className="skill-checks-running" data-testid={`skill-checks-running-${chat.session}`}>
      <p className="honest">
        <Badge variant="accent">Running</Badge>{" "}
        {`A chat ${chatPlace(chat.link.level, plane)} is ${doing} · ${runningSaid(chat.link.started, now)}`}
      </p>
      <ButtonRow size="sm">
        <Button size="sm" tooltip={`Shows ${chat.name}`} onPress={() => show(chat.session)}>
          {OPEN_CHAT}
        </Button>
        <Button size="sm" tooltip={`Asks to stop ${chat.name}`} onPress={() => stop(chat.session)}>
          Stop it
        </Button>
      </ButtonRow>
    </div>
  );
}
