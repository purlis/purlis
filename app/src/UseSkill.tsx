// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, memo, useContext, useEffect, useRef, useState } from "react";
import { Sparkles, TriangleAlert } from "lucide-react";
import * as Dialog from "./overlays/dialog";
import { useFocusBack } from "./EndingChat";
import { Field, SettingActions, SettingRow } from "./settings/components";
import { counted } from "./dispatches";
import { useSkillStudioOn } from "./skillStudio";
import {
  commands,
  type ChatSkill,
  type ChatSkills,
  type PlaneId,
  type SkillBudget,
} from "./bindings";

/** The budget in a line: `12 skills · 3,400 of 6,000 characters`. */
export function budgetSaid(budget: SkillBudget): string {
  const chars = `${budget.chars.toLocaleString("en-US")} of ${budget.most_chars.toLocaleString("en-US")} characters`;
  const skills = `${counted(budget.skills, "skill", "skills")} of ${budget.most_skills}`;
  const manual =
    budget.manual === 0
      ? ""
      : ` · ${counted(budget.manual, "manual-only skill", "manual-only skills")} free`;
  return `${skills} · ${chars}${manual}`;
}

/**
 * What the chat warns of where its set is past the budget, or nothing within it: which part is
 * over, and what that means for the chat.
 */
export function overSaid(budget: SkillBudget): string | undefined {
  const what =
    budget.over === "both"
      ? "too many skills and too much description"
      : budget.over === "skills"
        ? "too many skills"
        : budget.over === "chars"
          ? "too much description"
          : undefined;
  if (what === undefined) return undefined;
  return `This chat lists ${what} for purlis's skill budget, so some may be cut from what it reads. Make some skills manual-only, or turn some off for this workspace.`;
}

/** Whether a skill asks for anything before it is typed. */
export const takesArguments = (skill: ChatSkill) =>
  skill.arguments.length > 0 || skill.hint !== null;

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
 * name, which is what this does, and it costs nothing in the budget.
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
  const handBack = useFocusBack();
  const listed = skills?.skills ?? [];
  const shown = skillsMatching(listed, query);
  const budget = skills?.budget ?? null;
  const warning = budget === null ? undefined : overSaid(budget);

  // The first box of whatever is on screen takes the keys: the filter, or the first argument.
  useEffect(() => {
    content.current?.querySelector<HTMLElement>("input")?.focus();
  }, [picked]);

  const pick = (skill: ChatSkill) => {
    if (busy) return;
    if (!takesArguments(skill)) {
      onUse(skill.name, []);
      return;
    }
    setPicked(skill);
    setValues(skill.arguments.length > 0 ? skill.arguments.map(() => "") : [""]);
  };

  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open) onCancel();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="ui-scrim" />
        <Dialog.Content
          ref={content}
          className="ui-dialog use-skill"
          data-testid="use-skill"
          aria-describedby={undefined}
          onInteractOutside={(e) => e.preventDefault()}
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            content.current?.querySelector<HTMLElement>("input")?.focus();
          }}
          onCloseAutoFocus={handBack}
        >
          <Dialog.Title>{picked === undefined ? "Use skill" : `Use ${picked.name}`}</Dialog.Title>
          <p className="where">
            In <code>{chat}</code>. It is typed for you to send.
          </p>
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
              <SettingRow
                label="Find a skill"
                control={(ids) => (
                  <Field
                    ids={ids}
                    kind="text"
                    value={query}
                    placeholder="Name or what it does"
                    onChange={setQuery}
                  />
                )}
              />
              {skills === undefined ? (
                <p className="honest">Reading this chat&apos;s skills…</p>
              ) : listed.length === 0 ? (
                <p className="honest" data-testid="use-skill-none">
                  {skills.harness === null
                    ? "This chat runs no tool purlis knows, so it has no skills to use here."
                    : "This chat has no skills. Open Skill studio from the palette to see why any skill is left out."}
                </p>
              ) : shown.length === 0 ? (
                <p className="honest">No skill of this chat matches that.</p>
              ) : (
                <ul className="use-skill-list" aria-label="Skills">
                  {shown.map((skill) => (
                    <li key={skill.name}>
                      <button
                        type="button"
                        tabIndex={0}
                        className="ui-button ui-button-ghost use-skill-row"
                        data-testid="use-skill-row"
                        disabled={busy}
                        onClick={() => pick(skill)}
                      >
                        <span className="use-skill-name">{skill.name}</span>
                        <span className="use-skill-level">{skill.label}</span>
                        {skill.manual && <span className="use-skill-manual">manual only</span>}
                        <span className="use-skill-description">{skill.description}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              )}
              {trouble && (
                <p className="trouble" role="alert">
                  {trouble}
                </p>
              )}
              <SettingActions>
                <button className="ui-button" type="button" tabIndex={0} onClick={onCancel}>
                  Cancel
                </button>
              </SettingActions>
            </>
          ) : (
            <form
              onSubmit={(event) => {
                event.preventDefault();
                if (!busy) onUse(picked.name, values);
              }}
            >
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
              {trouble && (
                <p className="trouble" role="alert">
                  {trouble}
                </p>
              )}
              <SettingActions>
                <button
                  className="ui-button ui-button-primary"
                  type="submit"
                  tabIndex={0}
                  disabled={busy}
                >
                  {busy ? "Typing…" : "Type it"}
                </button>
                <button
                  className="ui-button"
                  type="button"
                  tabIndex={0}
                  onClick={() => setPicked(undefined)}
                >
                  Back
                </button>
              </SettingActions>
            </form>
          )}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
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

/**
 * **The Skills chip on a chat's pane** (#2775), in its top line beside the gauge: the chat's
 * skill budget at a glance, in the warning colour where its set is past it, and the way to Use
 * skill… for this chat. Read when the pane shows the chat. Nothing while the Skill studio is off,
 * or for a chat purlis knows no skills of.
 */
export const SkillsChip = memo(function SkillsChip({
  plane,
  session,
}: {
  plane: PlaneId;
  session: number;
}) {
  const on = useSkillStudioOn();
  const open = useOpenUseSkill();
  const [budget, setBudget] = useState<SkillBudget | null>(null);
  useEffect(() => {
    if (!on) return;
    let live = true;
    void commands
      .chatSkills(plane, session)
      .then((said) => {
        if (live) setBudget(said.status === "ok" ? said.data.budget : null);
      })
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [on, plane, session]);
  if (!on || budget === null) return null;
  const warning = overSaid(budget);
  const said = `${budgetSaid(budget)}. Use one of this chat's skills.`;
  return (
    <button
      type="button"
      className={`ui-button ui-button-ghost ui-button-sm pane-skills${warning === undefined ? "" : " over"}`}
      data-testid="pane-skills"
      title={warning === undefined ? said : `${warning} ${said}`}
      aria-label={warning === undefined ? said : `${warning} ${said}`}
      onClick={() => open(session)}
    >
      {warning === undefined ? (
        <Sparkles aria-hidden="true" />
      ) : (
        <TriangleAlert aria-hidden="true" />
      )}
      <span>{counted(budget.skills + budget.manual, "skill", "skills")}</span>
    </button>
  );
});
