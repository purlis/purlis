// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  memo,
  useCallback,
  useContext,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";
import { Sparkles, TriangleAlert } from "lucide-react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";
import { ListFilter, noMatchSaid } from "./ui/list/ListFilter";
import { ListBox, ListOption } from "./ui/list/List";
import { RowBelow, RowDescription, RowLabel } from "./ui/list/Row";
import { Button } from "./ui/button/Button";
import { counted } from "./dispatches";
import {
  AT_THE_ROOT,
  pinnerSaid,
  scopeOfPinner,
  SKILL_STUDIO_TITLE,
  skillStudioTitle,
  useSkillStudioOn,
} from "./skillStudio";
import { SKILLS_MISSING_PUT_AWAY, askChatSkills, type ChatSkillsAsk } from "./chatSkillsAsk";
import { chatNeedsOf, type ChatSkillsSaid } from "./chatSkillNeeds";
import type { SkillsChangedHere } from "./PlaneUpdated";
import {
  commands,
  type ChatSkill,
  type ChatSkills,
  type PlaneId,
  type SkillBudget,
  type SkillPinner,
} from "./bindings";

/** The budget in a line: `12 skills of 30 · 3,400 of 6,000 characters`, and the manual-only
 *  skills, free on every chat (SK-8). */
export function budgetSaid(budget: SkillBudget): string {
  const chars = `${budget.chars.toLocaleString("en-US")} of ${budget.most_chars.toLocaleString("en-US")} characters`;
  const skills = `${counted(budget.skills, "skill", "skills")} of ${budget.most_skills}`;
  const manual = counted(budget.manual, "manual-only skill", "manual-only skills");
  const free = budget.manual === 0 ? "" : ` · ${manual} free`;
  const hidden =
    budget.hidden === 0 ? "" : ` · ${counted(budget.hidden, "skill", "skills")} in the index skill`;
  return `${skills} · ${chars}${free}${hidden}`;
}

/**
 * What the chat warns of where its set is at four fifths of the budget or past it, or nothing
 * below: which part, and what can be done.
 */
export function warnsSaid(budget: SkillBudget): string | undefined {
  const what =
    budget.warns === "both"
      ? "skills and their descriptions are"
      : budget.warns === "skills"
        ? "skills are"
        : budget.warns === "chars"
          ? "skills' descriptions are"
          : undefined;
  if (what === undefined) return undefined;
  if (budget.still_over === "not-hideable")
    return `purlis's own skills and the ones this chat's program and repos bring are past purlis's skill budget by themselves, so all of them are listed. Turn some of the program's or repos' skills off in the Skill studio.`;
  if (budget.still_over === "pins")
    return `This chat's pinned skills are past purlis's skill budget, so all of them are listed. Unpin some in ${unpinIn(budget)}, or make some manual-only.`;
  if (budget.hidden > 0)
    return `This chat's ${what} past purlis's skill budget, so ${counted(budget.hidden, "skill is", "skills are")} hidden behind the index skill, which the chat loads them from. Make some skills manual-only, or turn some off for this workspace.`;
  return `This chat's ${what} near or past purlis's skill budget, past which some are hidden behind the index skill. Make some skills manual-only, or turn some off for this workspace.`;
}

/**
 * **A chat whose workspace's and persona's pins pass the most it reads only together** (#3391):
 * each list alone is accepted, so neither studio says anything is wrong, but the chat reads none
 * of either. Names both, how many each pins, and where to unpin; nothing where its pins are read.
 */
export function pinsTogetherSaid(budget: SkillBudget): string | undefined {
  const together = budget.pins_together;
  if (together === null) return undefined;
  const held = together.held
    .map((one) => `${pinnerSaid(one.by)} pins ${counted(one.pins, "skill", "skills")}`)
    .join(" and ");
  const where = studiosOf(together.held.map((one) => one.by));
  return `This chat reads none of its pins: ${held}, more than the ${together.most} a chat reads together. Unpin some in ${where}.`;
}

/** Where a chat's pins are undone (SK-15d): the studio of each of its workspace and persona that
 *  pins a skill, by its tab's title, or the studio itself where the window was not told. */
function unpinIn(budget: SkillBudget): string {
  return budget.pinned_by.length === 0 ? SKILL_STUDIO_TITLE : studiosOf(budget.pinned_by);
}

/** Each pinner's studio, by its tab's title: `Skill studio · ops or Skill studio · reviewer`. */
function studiosOf(pinners: readonly SkillPinner[]): string {
  return pinners.map((by) => skillStudioTitle(scopeOfPinner(by))).join(" or ");
}

/** What the filter over a chat's skills calls them. */
const SKILLS = { one: "skill", many: "skills" };

/** The skills whose name or description holds every word of `query`, in their order. */
export function skillsMatching(skills: readonly ChatSkill[], query: string): ChatSkill[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return skills.filter((skill) => {
    const text = `${skill.name} ${skill.description}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/**
 * **The skills Use skill… lists, in its order** (S8, #3694): only the ones this chat can use,
 * which is its set, with its built-in skills last, each part in the order the core gave.
 */
export function pickable(skills: readonly ChatSkill[]): ChatSkill[] {
  return [
    ...skills.filter((skill) => skill.level !== "built-in"),
    ...skills.filter((skill) => skill.level === "built-in"),
  ];
}

/**
 * **Use skill…** (#2775; spec #3679 S8, #3694): a quick picker, like the command palette, from
 * the palette, a chat tab's menu, a Skills view card's Use and a skill page's Use in this chat.
 * It lists only the chat's own skills, exactly its set, with built-in skills last, filtered as
 * the person types: Enter takes the first match, and the arrow keys walk the list. The chat's
 * budget, routing and pins are not said here: they are in the Skills view's header and its
 * Needs you, the one answer to what the chat gets.
 *
 * **Picking one types it into the chat in the way the chat's tool expects, and sends nothing**:
 * the person reads the line in the chat and presses Enter themselves. A skill that declares its
 * arguments asks for them in a second step, one box each, in the order it takes them; a skill
 * that only says what to pass gets one box with that hint. Nothing is checked here: what a skill
 * accepts is the skill's, and a chat that cannot be typed into now gets the line on the
 * clipboard, with why (the core's sentence).
 *
 * A manual-only skill is listed with the others and marked: it runs only when it is asked for by
 * name, which is what this does.
 *
 * **Opened on one skill** (`picks`, the Skills view's Use in this chat, #3266), it is that skill
 * picked as the chat's skills arrive: typed at once where it asks for nothing, its boxes where it
 * does. One the chat does not have leaves the list, filtered to its name, saying so.
 */
export function UseSkill({
  chat,
  picks,
  skills,
  trouble,
  busy,
  onUse,
  onCancel,
}: {
  /** The chat it types into, as its tab shows it. */
  chat: string;
  /** The skill it opens on, by name, where one was asked for. */
  picks?: string;
  /** What the chat has, or `undefined` while it is being read. */
  skills?: ChatSkills;
  /** Why the last pick typed nothing, in the core's sentence. */
  trouble?: string;
  /** Whether a pick is being typed now, so it is not typed twice. */
  busy: boolean;
  /** Types `skill` with `values`, one per declared argument in order. */
  onUse: (skill: string, values: string[]) => void;
  onCancel: () => void;
}) {
  const [query, setQuery] = useState(picks ?? "");
  const named = picks === undefined ? undefined : skills?.skills.find((one) => one.name === picks);
  const [picked, setPicked] = useState<ChatSkill | undefined>(() =>
    named?.takes_arguments ? named : undefined,
  );
  const [values, setValues] = useState<string[]>(() => (named === undefined ? [] : boxesOf(named)));
  const content = useRef<HTMLDivElement>(null);
  const listId = useId();
  const listed = pickable(skills?.skills ?? []);
  const shown = skillsMatching(listed, query);

  // **The skill it was opened on** (`picks`): drawn picked where it asks for something, typed at
  // once where it asks for nothing. The window draws this dialog again once the chat's skills are
  // read (its key), so what it starts with is decided once, from them.
  const typed = useRef(false);
  useEffect(() => {
    if (named === undefined || named.takes_arguments || typed.current) return;
    typed.current = true;
    onUse(named.name, []);
  }, [named, onUse]);

  // The first box of whatever is on screen takes the keys: the filter, or the first argument.
  useEffect(() => {
    content.current?.querySelector<HTMLElement>("input")?.focus();
  }, [picked, skills]);

  const pick = (skill: ChatSkill) => {
    if (busy) return;
    if (!skill.takes_arguments) {
      onUse(skill.name, []);
      return;
    }
    setPicked(skill);
    setValues(boxesOf(skill));
  };

  /** **The filter's keys, as a palette's** (S8): Enter takes the first match, and the down arrow
   *  goes into the list, where the arrows walk it and Enter takes the one on. */
  const filterKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") {
      event.preventDefault();
      const first = shown[0];
      if (first !== undefined) pick(first);
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      document
        .getElementById(listId)
        ?.querySelector<HTMLElement>('[role="option"]:not([aria-disabled="true"])')
        ?.focus();
    }
  };

  // **On the dialog family** (#2910): picking a skill that takes arguments asks for them in
  // place, and Back returns to the list. Its answers are the footer's: Cancel, Back and Type it.
  return (
    <Dialog
      onClose={onCancel}
      title={picked === undefined ? "Use skill" : `Use ${picked.name}`}
      subtitle={
        <>
          In <code>{chat}</code>. It is typed for you to send.
        </>
      }
      className="use-skill"
      data-testid="use-skill"
      {...(picked === undefined
        ? {}
        : {
            alternative: { children: "Back", onPress: () => setPicked(undefined) },
            main: {
              children: busy ? "Typing…" : "Type it",
              reserve: ["Type it", "Typing…"],
              isDisabled: busy,
              onPress: () => onUse(picked.name, values),
            },
          })}
    >
      <div ref={content}>
        {picked === undefined ? (
          <>
            {/* A quick picker's box, on every list however short: typing is how it is used. */}
            {listed.length > 0 && (
              <ListFilter
                noun={SKILLS}
                total={listed.length}
                query={query}
                onQuery={setQuery}
                onKeyDown={filterKey}
                controls={listId}
              />
            )}
            <div id={listId}>
              {skills === undefined ? (
                <p className="honest">Reading this chat&apos;s skills…</p>
              ) : listed.length === 0 ? (
                <p className="honest" data-testid="use-skill-none">
                  {!skills.known
                    ? (skills.not_supported ??
                      "This chat runs no tool purlis knows, so it has no skills to use here.")
                    : "This chat has no skills. Its skills chip shows why any skill is left out."}
                </p>
              ) : shown.length === 0 ? (
                <p className="honest">{noMatchSaid(SKILLS, query)}</p>
              ) : (
                // A list to pick from (`ui/list`, #2911): a press or Enter on a skill picks it.
                <ListBox
                  className="use-skill-list"
                  aria-label="Skills"
                  size="lg"
                  disabledKeys={busy ? shown.map((skill) => skill.name) : []}
                  onAction={(key) => {
                    const skill = shown.find((one) => one.name === key);
                    if (skill !== undefined) pick(skill);
                  }}
                >
                  {shown.map((skill) => (
                    <ListOption
                      key={skill.name}
                      id={skill.name}
                      textValue={skill.name}
                      className="use-skill-row"
                      data-testid="use-skill-row"
                    >
                      <RowLabel className="use-skill-name">{skill.name}</RowLabel>
                      {/* Its one line about it: where it comes from, and whether only a person
                          can use it. */}
                      <RowDescription>
                        <span className="use-skill-level">{skill.label}</span>
                        {skill.manual && <span className="use-skill-manual"> · manual only</span>}
                        {skill.hidden !== null && (
                          <span className="use-skill-hidden"> · in the index skill</span>
                        )}
                      </RowDescription>
                      <RowBelow className="use-skill-description">{skill.description}</RowBelow>
                    </ListOption>
                  ))}
                </ListBox>
              )}
            </div>
          </>
        ) : (
          <>
            <p className="use-skill-description">{picked.description}</p>
            {(picked.arguments.length > 0 ? picked.arguments : ["Arguments"]).map((name, at) => (
              <SettingRow
                key={name}
                label={name}
                help={picked.arguments.length === 0 ? (picked.hint ?? undefined) : undefined}
                control={(ids) => (
                  <Field
                    ids={ids}
                    kind="text"
                    value={values[at] ?? ""}
                    placeholder={
                      picked.arguments.length > 0 && at === 0
                        ? (picked.hint ?? undefined)
                        : undefined
                    }
                    onChange={(to) =>
                      setValues((was) => was.map((one, i) => (i === at ? to : one)))
                    }
                  />
                )}
              />
            ))}
          </>
        )}
        {trouble && (
          <p className="trouble" role="alert">
            {trouble}
          </p>
        )}
      </div>
    </Dialog>
  );
}

/** The boxes a skill's arguments start in: one empty box per argument, or the one of its hint. */
function boxesOf(skill: ChatSkill): string[] {
  return skill.arguments.length > 0 ? skill.arguments.map(() => "") : [""];
}

/** Opens Use skill… for chat `session`: the one way in, for the catalogue's rows and the chip. */
export type OpenUseSkill = (session: number, picks?: string) => void;

const Opener = createContext<OpenUseSkill>(() => undefined);

/** Hands every pane of a project the way to open Use skill… ({@link OpenUseSkill}). */
export const UseSkillOpener = Opener.Provider;

/** {@link OpenUseSkill}, inside a project's panes. Outside one it opens nothing. */
export function useOpenUseSkill(): OpenUseSkill {
  return useContext(Opener);
}

/** What the window holds of each open chat's skill set, by session (#2775, #3953): what the
 *  Skills view's Needs you says of the chat as a whole, its budget known. A session that is not
 *  here has none known yet. */
export type ChatSkillSets = Readonly<Record<number, ChatSkillsSaid & { budget: SkillBudget }>>;

/** The sets of a project's chats, for the chip on each pane (`SkillsChip`). */
export const ChatSkillSetsHere = createContext<ChatSkillSets>({});

/** How many skills a chat's set holds, whatever its listing leaves out. */
export const skillsInTheSet = (budget: SkillBudget) => budget.skills + budget.manual;

/** What one `chat_skills` answer leaves the window holding of a chat's set: its budget, how its
 *  router did at its start and what it started without. */
type HeldSet = Omit<ChatSkillsSaid, "changed"> & { budget: SkillBudget };

/**
 * **The skill sets of chats `sessions` of `plane`** (#2775, #3953), read once each while the
 * Skill studio is on, and how to set one read elsewhere (Use skill… reads a chat's skills as it
 * opens). The one place a window holds them: the chip on each pane draws it.
 *
 * Each is the one `chat_skills` answer the chat's budget always came from, which says too how its
 * router did at its start and what it started without, both held in the core from its start: no
 * read is added to a chat's open. What changed since it started is `changed`, the window's own
 * answer from `chats_plane_updated` (`usePlaneUpdated`), which its tab's mark reads too.
 */
export function useChatSkillSets(
  plane: PlaneId,
  sessions: readonly number[],
  on: boolean,
  changed: SkillsChangedHere = NOTHING_CHANGED,
): [ChatSkillSets, (session: number, said: ChatSkills | null) => void] {
  const [held, setHeld] = useState<Readonly<Record<number, HeldSet>>>({});
  const asked = useRef(new Set<number>());
  const set = useCallback((session: number, said: ChatSkills | null) => {
    setHeld((was) => {
      const budget = said?.budget ?? null;
      if (said !== null && budget !== null)
        return {
          ...was,
          [session]: { budget, routing: said.routing, missing: said.missing },
        };
      if (!(session in was)) return was;
      return Object.fromEntries(
        Object.entries(was).filter(([one]) => Number(one) !== session),
      ) as Record<number, HeldSet>;
    });
  }, []);
  const key = sessions.join(",");
  useEffect(() => {
    if (!on) {
      // Read again, all of them, once the studio is on again.
      asked.current.clear();
      return;
    }
    for (const session of key === "" ? [] : key.split(",").map(Number)) {
      if (asked.current.has(session)) continue;
      asked.current.add(session);
      void commands
        .chatSkills(plane, session)
        .then((said) => set(session, said.status === "ok" ? said.data : null))
        .catch(() => undefined);
    }
  }, [key, on, plane, set]);
  // A "started without" notice put away is put away in the core too: the chip says it no more.
  useEffect(() => {
    const putAway = (event: Event) => {
      const { detail } = event as CustomEvent<ChatSkillsAsk>;
      if (detail.plane !== plane) return;
      setHeld((was) => {
        const one = was[detail.session];
        if (one === undefined || one.missing === null) return was;
        return { ...was, [detail.session]: { ...one, missing: null } };
      });
    };
    window.addEventListener(SKILLS_MISSING_PUT_AWAY, putAway);
    return () => window.removeEventListener(SKILLS_MISSING_PUT_AWAY, putAway);
  }, [plane]);
  const sets = useMemo<ChatSkillSets>(
    () =>
      Object.fromEntries(
        Object.entries(held).map(([session, one]) => [
          session,
          { ...one, changed: changed[Number(session)] ?? null },
        ]),
      ),
    [held, changed],
  );
  return [on ? sets : NO_SETS, set];
}

/** No chat's set known: what the window holds while the Skill studio is off. */
const NO_SETS: ChatSkillSets = {};

/** No chat's skills changed since it started. */
const NOTHING_CHANGED: SkillsChangedHere = {};

/** What the Skills chip's tooltip says it does (S8, #3694). */
export const SHOW_THIS_CHATS_SKILLS = "Show this chat's skills";

/**
 * **The Skills chip on a chat's pane** (#2775; spec #3679 S8, #3694, #3953), in its top line
 * beside the gauge: how many skills the chat's set holds, warning-coloured whenever the Skills
 * view's Needs you holds a card of the chat's own (`chatNeedsOf`, the view's own reckoning): its
 * set near or past the budget, its pins read by none, its router fallen back at its start, skills
 * it started without, or its skills changed since it started. Its tooltip names each. Pressed, it
 * shows this chat's skills in the sidebar Skills view, the one answer to what the chat gets and
 * why, where each need has its fix; it no longer opens Use skill…. Its set is the window's
 * ({@link ChatSkillSetsHere}). Nothing while the Skill studio is off, or for a chat purlis knows
 * no skills of; where the pane leaves the chip out, the Skills view says the budget (`⌘⇧X`).
 */
export const SkillsChip = memo(function SkillsChip({
  plane,
  session,
}: {
  plane: PlaneId;
  session: number;
}) {
  const on = useSkillStudioOn();
  const said = useContext(ChatSkillSetsHere)[session];
  if (!on || said === undefined) return null;
  // The scope only aims each need's fix, which the view offers and the chip does not.
  const needs = chatNeedsOf(said, AT_THE_ROOT);
  const warning = needs.length === 0 ? undefined : needs.map((one) => one.title).join(" · ");
  const tip =
    warning === undefined ? `${SHOW_THIS_CHATS_SKILLS}.` : `${warning}. ${SHOW_THIS_CHATS_SKILLS}.`;
  const count = counted(skillsInTheSet(said.budget), "skill", "skills");
  return (
    <Button
      variant={warning === undefined ? "ghost" : "warning-soft"}
      size="sm"
      className="pane-skills"
      data-testid="pane-skills"
      data-over={warning === undefined ? undefined : ""}
      tooltip={tip}
      aria-label={`${count}. ${tip}`}
      icon={
        warning === undefined ? (
          <Sparkles aria-hidden="true" />
        ) : (
          <TriangleAlert aria-hidden="true" />
        )
      }
      onPress={() => askChatSkills(plane, session)}
    >
      {count}
    </Button>
  );
});
