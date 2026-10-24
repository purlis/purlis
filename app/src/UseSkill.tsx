// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  createContext,
  memo,
  useCallback,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
} from "react";
import { Sparkles, TriangleAlert } from "lucide-react";
import { Field, SettingRow } from "./settings/components";
import { Dialog } from "./ui/dialog/Dialog";
import { FILTERED_FROM, ListFilter, noMatchSaid } from "./ui/list/ListFilter";
import { ListBox, ListOption } from "./ui/list/List";
import { RowBelow, RowDescription, RowLabel } from "./ui/list/Row";
import { Button } from "./ui/button/Button";
import { counted } from "./dispatches";
import { useSkillStudioOn } from "./skillStudio";
import {
  commands,
  type ChatSkill,
  type ChatSkills,
  type PlaneId,
  type SkillBudget,
} from "./bindings";

/** The budget in a line: `12 skills of 30 · 3,400 of 6,000 characters`, and the manual-only
 *  skills, free on every chat (SK-8). */
export function budgetSaid(budget: SkillBudget): string {
  const chars = `${budget.chars.toLocaleString("en-US")} of ${budget.most_chars.toLocaleString("en-US")} characters`;
  const skills = `${counted(budget.skills, "skill", "skills")} of ${budget.most_skills}`;
  const manual = counted(budget.manual, "manual-only skill", "manual-only skills");
  const free = budget.manual === 0 ? "" : ` · ${manual} free`;
  return `${skills} · ${chars}${free}`;
}

/** The budget in a few words, for a chat tab's menu row: `12 of 30 skills`, and that it is near
 *  or past the budget where it warns. */
export function budgetShort(budget: SkillBudget): string {
  const short = `${budget.skills} of ${budget.most_skills} skills`;
  return budget.warns === null ? short : `${short}, near the budget`;
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
  return `This chat's ${what} near or past purlis's skill budget, past which some are hidden from what it reads. Make some skills manual-only, or turn some off for this workspace.`;
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
 * **Use skill…** (#2775), from the palette, a chat tab's menu or the Skills chip on its pane: the
 * chat's own skills, exactly its set and nothing else, with the chat's skill budget above them.
 *
 * **Picking one types it into the chat in the way the chat's tool expects, and sends nothing**:
 * the person reads the line in the chat and presses Enter themselves. A skill that declares its
 * arguments asks for them first, one box each, in the order it takes them; a skill that only
 * says what to pass gets one box with that hint. Nothing is checked here: what a skill accepts
 * is the skill's, and a chat that cannot be typed into now gets the line on the clipboard, with
 * why (the core's sentence).
 *
 * A manual-only skill is listed with the others and marked: it runs only when it is asked for by
 * name, which is what this does. It costs nothing in the budget: every chat leaves it out of its
 * listing.
 */
export function UseSkill({
  chat,
  skills,
  trouble,
  busy,
  onUse,
  onCancel,
}: {
  /** The chat it types into, as its tab shows it. */
  chat: string;
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
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<ChatSkill>();
  const [values, setValues] = useState<string[]>([]);
  const content = useRef<HTMLDivElement>(null);
  const listId = useId();
  const listed = skills?.skills ?? [];
  const shown = skillsMatching(listed, query);
  const budget = skills?.budget ?? null;
  const warning = budget === null ? undefined : warnsSaid(budget);

  // The first box of whatever is on screen takes the keys: the filter, or the first argument.
  useEffect(() => {
    content.current?.querySelector<HTMLElement>("input")?.focus();
  }, [picked]);

  const pick = (skill: ChatSkill) => {
    if (busy) return;
    if (!skill.takes_arguments) {
      onUse(skill.name, []);
      return;
    }
    setPicked(skill);
    setValues(skill.arguments.length > 0 ? skill.arguments.map(() => "") : [""]);
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
        {budget !== null && (
          <p className="use-skill-budget" data-testid="use-skill-budget">
            {budgetSaid(budget)}
          </p>
        )}
        {warning !== undefined && (
          <p className="honest use-skill-over" role="status">
            <TriangleAlert aria-hidden="true" /> {warning}
          </p>
        )}

        {picked === undefined ? (
          <>
            {listed.length >= FILTERED_FROM && (
              <ListFilter
                noun={SKILLS}
                total={listed.length}
                query={query}
                onQuery={setQuery}
                controls={listId}
              />
            )}
            <div id={listId}>
              {skills === undefined ? (
                <p className="honest">Reading this chat&apos;s skills…</p>
              ) : listed.length === 0 ? (
                <p className="honest" data-testid="use-skill-none">
                  {!skills.known
                    ? "This chat runs no tool purlis knows, so it has no skills to use here."
                    : "This chat has no skills. Open Skill studio from the palette to see why any skill is left out."}
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

/** Opens Use skill… for chat `session`: the one way in, for the catalogue's rows and the chip. */
export type OpenUseSkill = (session: number) => void;

const Opener = createContext<OpenUseSkill>(() => undefined);

/** Hands every pane of a project the way to open Use skill… ({@link OpenUseSkill}). */
export const UseSkillOpener = Opener.Provider;

/** {@link OpenUseSkill}, inside a project's panes. Outside one it opens nothing. */
export function useOpenUseSkill(): OpenUseSkill {
  return useContext(Opener);
}

/** Each open chat's skill budget, by session: read once a chat is open, and again as Use skill…
 *  reads its skills. A session that is not here has none known yet. */
export type SkillBudgets = Readonly<Record<number, SkillBudget>>;

/** The budgets of a project's chats, for the chip on each pane (`SkillsChip`). */
export const SkillBudgetsHere = createContext<SkillBudgets>({});

/** How many skills a chat's set holds, whatever its listing leaves out. */
export const skillsInTheSet = (budget: SkillBudget) => budget.skills + budget.manual;

/**
 * **The skill budgets of chats `sessions` of `plane`** (#2775), read once each while the Skill
 * studio is on, and how to set one read elsewhere (Use skill… reads a chat's skills as it opens).
 * The one place a window holds them: the chip on each pane and each chat tab's menu row draw it.
 */
export function useChatSkillBudgets(
  plane: PlaneId,
  sessions: readonly number[],
  on: boolean,
): [SkillBudgets, (session: number, budget: SkillBudget | null) => void] {
  const [budgets, setBudgets] = useState<SkillBudgets>({});
  const asked = useRef(new Set<number>());
  const set = useCallback((session: number, budget: SkillBudget | null) => {
    setBudgets((was) => {
      if (budget !== null) return { ...was, [session]: budget };
      if (!(session in was)) return was;
      return Object.fromEntries(
        Object.entries(was).filter(([one]) => Number(one) !== session),
      ) as SkillBudgets;
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
        .then((said) => set(session, said.status === "ok" ? said.data.budget : null))
        .catch(() => undefined);
    }
  }, [key, on, plane, set]);
  return [on ? budgets : NO_BUDGETS, set];
}

/** No chat's budget known: what the window holds while the Skill studio is off. */
const NO_BUDGETS: SkillBudgets = {};

/**
 * **The Skills chip on a chat's pane** (#2775), in its top line beside the gauge: the chat's
 * skill budget at a glance, in the warning look where its set is near or past it, and the way to
 * Use skill… for this chat. Its budget is the window's ({@link SkillBudgetsHere}). Nothing while
 * the Skill studio is off, or for a chat purlis knows no skills of; where the pane leaves the chip
 * out, the chat tab's menu row says the budget.
 */
export const SkillsChip = memo(function SkillsChip({ session }: { session: number }) {
  const on = useSkillStudioOn();
  const open = useOpenUseSkill();
  const budget = useContext(SkillBudgetsHere)[session];
  if (!on || budget === undefined) return null;
  const warning = warnsSaid(budget);
  const said = `${budgetSaid(budget)}. Use one of this chat's skills.`;
  return (
    <Button
      variant={warning === undefined ? "ghost" : "warning-soft"}
      size="sm"
      className="pane-skills"
      data-testid="pane-skills"
      data-over={warning === undefined ? undefined : ""}
      tooltip={warning === undefined ? said : `${warning} ${said}`}
      aria-label={warning === undefined ? said : `${warning} ${said}`}
      icon={
        warning === undefined ? (
          <Sparkles aria-hidden="true" />
        ) : (
          <TriangleAlert aria-hidden="true" />
        )
      }
      onPress={() => open(session)}
    >
      {counted(skillsInTheSet(budget), "skill", "skills")}
    </Button>
  );
});
