// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A chat the Skill studio started stays linked to its skill and job** (#5185;
 * `prototype/studio-chat-link`, variant A with B's Grades card, the operator's verdict of
 * 2026-11-04). The core keeps the link on the chat's own record (`reopen.json`'s
 * `chats[].studio`), so it outlives a relaunch; the window reads it with the chat
 * (`OpenChat.studio`) and when the studio starts one, and lends it here:
 *
 * - **the chat's header** names the skill and the job, and opens the skill's page
 *   ({@link StudioChatChip});
 * - **the skill's page** says the chat while it runs, above its tabs, with Open chat
 *   ({@link StudioChatLines}), and lists it on Runs afterwards ({@link StudioChatsStarted});
 * - **the Grades card** shows the running job in place of its button;
 * - **the draft** it wrote offers Open chat where it waits for review.
 *
 * Only chats still open are lent: a closed chat has no tab to open.
 */
import { Sparkles } from "lucide-react";
import { createContext, useContext, useMemo } from "react";
import type { SkillDraft, SkillLevel, StudioJobKind, StudioLink } from "./bindings";
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
import { Button } from "./ui/button/Button";
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
};

const NONE: StudioChatsLent = { chats: [], show: () => undefined, openSkill: () => undefined };

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

/** What the chat's chip says: `test-design · writing outcome evals`. */
export function chipSaid(link: StudioLink, done: boolean): string {
  return `${link.skill ?? "New skill"} · ${jobSaid(link.job, done)}`;
}

/** The chats started for the skill `name` at `level`, and no other's. */
export function chatsOfSkill(
  chats: readonly StudioChat[],
  name: string,
  level: SkillLevel,
): StudioChat[] {
  return chats.filter((one) => one.link.skill === name && sameLevel(one.link.level, level));
}

/** **Whether a studio chat's job is done**: a draft it wrote waits, or its program ended. */
export function isDone(chat: StudioChat, drafts: readonly SkillDraft[], ended: boolean): boolean {
  return ended || drafts.some((one) => one.chat === chat.session);
}

/** The studio's chats whose skill a level is: where a skill of `level` is seen. */
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
export function linkOfAsk(ask: SkillChatAsk): StudioLink | undefined {
  switch (ask.kind) {
    case "draft":
      return { skill: null, level: ask.level, job: "draft" };
    case "triggers":
      return { skill: ask.name, level: ask.level, job: "trigger-tests" };
    case "evals":
      return { skill: ask.name, level: ask.level, job: "outcome-evals" };
    case "checks":
      return { skill: ask.name, level: ask.level, job: "checks" };
    case "improve":
      return { skill: ask.name, level: ask.level, job: "improve" };
    case "redo":
      return { skill: ask.name, level: ask.level, job: "redo" };
    case "try":
      return undefined;
  }
}

/** Whether chat `session`'s program has ended, read off the chats' store. */
export function useEnded(session: number): boolean {
  return useChatsSelect(useChatsHere(), (states) => endedBy(stateOf(states, session)));
}

/**
 * **The chip in a studio chat's header** (#5185, variant A): `✦ test-design · writing outcome
 * evals`, which opens the skill's page. Nothing for a chat the studio did not start.
 */
export function StudioChatChip({ plane, session }: { plane: string; session: number }) {
  const { chats } = useStudioChats();
  const chat = chats.find((one) => one.session === session);
  return chat === undefined ? null : <ChipOf plane={plane} chat={chat} />;
}

/** The chip of a chat the studio started: done once its draft waits or its program ended. */
function ChipOf({ plane, chat }: { plane: string; chat: StudioChat }) {
  const { openSkill } = useStudioChats();
  const drafts = useSkillDrafts(plane, 0);
  const ended = useEnded(chat.session);
  const done = isDone(chat, drafts, ended);
  const where = chat.link.skill === null ? "Drafts" : chat.link.skill;
  return (
    <Button
      variant={done ? "ghost" : "soft-accent"}
      size="sm"
      className="pane-studio"
      data-testid="pane-studio"
      icon={<Sparkles aria-hidden="true" />}
      tooltip={`Open ${where} in the Skill studio`}
      onPress={() => openSkill(chat.link)}
    >
      {chipSaid(chat.link, done)}
    </Button>
  );
}

/** One running studio chat's line above a skill's tabs. */
function StudioChatLine({ chat, onShow }: { chat: StudioChat; onShow: () => void }) {
  const doing = jobSaid(chat.link.job, false);
  return (
    <Notice
      cause={`studio-chat:${chat.session}`}
      tone="news"
      label={`A chat is ${doing}`}
      fixes={[{ label: "Open chat", onPress: onShow }]}
    >
      <span data-testid={`studio-chat-running-${chat.session}`}>
        {`A chat ${chatPlace(chat.link.level)} is ${doing} · running`}
      </span>
    </Notice>
  );
}

/** A running chat's line, drawn only while its program runs and no draft of it waits. */
function RunningLine({
  chat,
  drafts,
  onShow,
}: {
  chat: StudioChat;
  drafts: readonly SkillDraft[];
  onShow: () => void;
}) {
  const ended = useEnded(chat.session);
  return isDone(chat, drafts, ended) ? null : <StudioChatLine chat={chat} onShow={onShow} />;
}

/**
 * **The line above a skill's tabs while a studio chat works on it** (#5185, variant A): `A chat
 * at purlis-demo is writing outcome evals · running`, with Open chat. Gone once it is done; the
 * Runs tab keeps the chat.
 */
export function StudioChatLines({
  name,
  level,
  drafts,
}: {
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
          chat={chat}
          drafts={drafts}
          onShow={() => show(chat.session)}
        />
      ))}
    </div>
  );
}

/** One row of the chats the studio started, with whether it is still running. */
function StartedRow({
  chat,
  drafts,
  onShow,
}: {
  chat: StudioChat;
  drafts: readonly SkillDraft[];
  onShow: () => void;
}) {
  const ended = useEnded(chat.session);
  const drafted = drafts.some((one) => one.chat === chat.session);
  const done = isDone(chat, drafts, ended);
  const job = jobSaid(chat.link.job, done);
  const said = `${job.charAt(0).toUpperCase()}${job.slice(1)}`;
  return (
    <ListRow id={String(chat.session)} textValue={said} data-testid={`studio-chat-${chat.session}`}>
      <RowLabel>{said}</RowLabel>
      <RowTrail>
        <Badge variant={done ? "ok" : "accent"}>{done ? "Finished" : "Running"}</Badge>
        <Button size="sm" tooltip={`Shows ${chat.name}`} onPress={onShow}>
          Open chat
        </Button>
      </RowTrail>
      <RowBelow clamp={1}>
        {`${chat.name}, ${chatPlace(chat.link.level)}${drafted ? " · its draft waits in Drafts" : ""}`}
      </RowBelow>
    </ListRow>
  );
}

/**
 * **"Chats the studio started"** on a skill's Runs (#5185, variant A): each open chat the studio
 * started for this skill, running or finished, with Open chat. Nothing where there is none.
 */
export function StudioChatsStarted({
  name,
  level,
  drafts,
}: {
  name: string;
  level: SkillLevel;
  drafts: readonly SkillDraft[];
}) {
  const { chats, show } = useStudioChats();
  const mine = useMemo(() => chatsOfSkill(chats, name, level), [chats, name, level]);
  if (mine.length === 0) return null;
  return (
    <section
      className="skill-page-studio-started"
      aria-labelledby="skill-studio-started"
      data-testid="skill-studio-started"
    >
      <PanelSectionTitle id="skill-studio-started">Chats the studio started</PanelSectionTitle>
      <List aria-labelledby="skill-studio-started" size="sm" onAction={(key) => show(Number(key))}>
        {mine.map((chat) => (
          <StartedRow
            key={chat.session}
            chat={chat}
            drafts={drafts}
            onShow={() => show(chat.session)}
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
  const ended = useChatsSelect(states, (now) =>
    ofKinds.map((one) => endedBy(stateOf(now, one.session))).join(","),
  ).split(",");
  return ofKinds.find((one, at) => !isDone(one, drafts, ended[at] === "true"));
}

/** **The Grades card's running job** (#5185): what the chat does and where, with Open chat. */
export function RunningJob({ chat }: { chat: StudioChat }) {
  const { show } = useStudioChats();
  return (
    <div className="skill-checks-running" data-testid={`skill-checks-running-${chat.session}`}>
      <p className="honest">
        <Badge variant="accent">Running</Badge>{" "}
        {`A chat ${chatPlace(chat.link.level)} is ${jobSaid(chat.link.job, false)}.`}
      </p>
      <Button size="sm" onPress={() => show(chat.session)}>
        Open chat
      </Button>
    </div>
  );
}
