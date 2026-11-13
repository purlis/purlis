// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type SkillBudgetPreview, type StudioBudget } from "./bindings";
import { counted } from "./dispatches";
import { capitalised } from "./chatEnded";
import { useReadOnlySaid } from "./ReadOnlyLine";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";
import { TextField } from "./ui/field/Field";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "./ui/table/Table";

/** The most a window sends: past it no chat could reach a budget, so it says the same. */
const MOST_SENT = 4_294_967_295;

/** How long typing rests before the dialog works out what it would do. */
const TYPING_RESTS_MS = 150;

/** How many skills a line of the dialog names before it counts the rest. */
const NAMES_SAID = 5;

/** What an invalid box says. There is no largest number: past a limit is a warning. */
export const BUDGET_INVALID =
  "Type a whole number of at least 1, or leave it empty for the number below it.";

/** What a chat does about a change, said where the change is made. */
export const BUDGET_APPLIES =
  "New chats get this. Open chats keep theirs and offer a restart once.";

/** What a box holds: a number; nothing, for the number below; or something that is neither. */
export function budgetTyped(text: string): number | null | undefined {
  const typed = text.trim();
  if (typed === "") return null;
  if (!/^[0-9]+$/.test(typed)) return undefined;
  const number = Number(typed);
  return number >= 1 ? Math.min(number, MOST_SENT) : undefined;
}

/** The names on a line of the dialog: `a, b and 3 more`. */
function namesSaid(names: readonly string[]): string {
  const said = names.slice(0, NAMES_SAID).join(", ");
  const more = names.length - NAMES_SAID;
  return more > 0 ? `${said} and ${more} more` : said;
}

/** **What the numbers typed would cost, a line each** (#5353): what new chats list and hide, which
 *  skills that moves, and about how many tokens that is per chat. */
export function budgetCostSaid(preview: SkillBudgetPreview): string[] {
  const moved = Math.abs(preview.more_tokens);
  const tokens = `${moved.toLocaleString("en-US")} ${preview.more_tokens > 0 ? "more" : "fewer"} ${moved === 1 ? "token" : "tokens"}`;
  return [
    `New chats list ${counted(preview.listed, "skill", "skills")} and hide ${preview.hidden}.`,
    ...(preview.now_listed.length === 0 ? [] : [`Now listed: ${namesSaid(preview.now_listed)}.`]),
    ...(preview.now_hidden.length === 0 ? [] : [`Now hidden: ${namesSaid(preview.now_hidden)}.`]),
    preview.more_tokens === 0 ? "No change in what each chat spends." : `About ${tokens} per chat.`,
  ];
}

/** Where a change is written: a workspace's, or the project's for none. */
export type BudgetLevel = { workspace: string | null; persona: string | null };

/** One program's numbers for the level, each a number or none for the number below. */
export type BudgetWrite = {
  program: StudioBudget;
  skills: number | null;
  chars: number | null;
  /** What is said once it is written. */
  said: string;
};

/** How a write goes: told what happened, to close a dialog or say why not. */
export type BudgetWriteTold = { done: () => void; refused: (why: string) => void };

/** **Writes straight to the core**, for a page with no notice of its own (Settings). */
export function writeBudgetTo(plane: string, level: BudgetLevel) {
  return (write: BudgetWrite, told: BudgetWriteTold): void => {
    void commands
      .skillBudgetSet(plane, level.workspace, write.program.key, write.skills, write.chars)
      .then((answer) => {
        if (answer.status === "error") told.refused(answer.error);
        else told.done();
      })
      .catch((err: unknown) => told.refused(String(err)));
  };
}

/** What a reset did, as a sentence: in a workspace, the project's number may stand after it. */
function resetSaid(program: StudioBudget, what: string): string {
  return `This level no longer sets ${program.program}'s ${what}. New chats get the number below it.`;
}

/**
 * **Each program's skill budget, with where each number comes from** (#5353; ADR 0102 as amended
 * 2026-11-05): the Routing page's table and Settings' In force. Each row has Change…, and each
 * number Reset to default, which is unavailable where this level writes none. Settings keeps the
 * numbers, whichever of the two wrote them.
 */
export function SkillBudgetTable({
  plane,
  budget,
  level,
  extensionRouter = false,
  onWrite,
}: {
  plane: string;
  budget: readonly StudioBudget[];
  level: BudgetLevel;
  /** A router extension routes these chats: what the dialog works out is the built-in router's. */
  extensionRouter?: boolean;
  onWrite: (write: BudgetWrite, told: BudgetWriteTold) => void;
}) {
  const [changing, setChanging] = useState<string>();
  const [refused, setRefused] = useState<string>();
  const readOnly = useReadOnlySaid();
  const off = readOnly !== undefined;
  const open = budget.find((one) => one.key === changing);
  const here = level.workspace === null ? "this project" : "this workspace";
  const unavailable = (written: number | null) =>
    readOnly ?? (written === null ? `${capitalised(here)} sets no number of its own.` : undefined);
  const reset = (write: BudgetWrite) => {
    setRefused(undefined);
    onWrite(write, { done: () => undefined, refused: setRefused });
  };
  return (
    <div className="skill-budget" data-testid="skill-budget">
      <Table aria-label="Skill budget on each program" stacks>
        <TableHeader>
          <TableColumn isRowHeader>Program</TableColumn>
          <TableColumn>Most skills</TableColumn>
          <TableColumn>Most characters</TableColumn>
          <TableColumn hideName>Change</TableColumn>
        </TableHeader>
        <TableBody>
          {budget.map((one) => (
            <TableRow key={one.key} id={one.key} textValue={one.program}>
              <TableCell heading="Program">{one.program}</TableCell>
              <TableCell heading="Most skills">
                <span className="skill-budget-number">
                  <span data-testid={`skill-budget-skills-${one.key}`}>{one.skills_said}</span>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="skill-budget-press"
                    aria-label={`Reset to default, ${one.program}'s most skills`}
                    isDisabled={off || one.skills_here === null}
                    tooltip={unavailable(one.skills_here)}
                    onPress={() =>
                      reset({
                        program: one,
                        skills: null,
                        chars: one.chars_here,
                        said: resetSaid(one, "most skills"),
                      })
                    }
                  >
                    Reset to default
                  </Button>
                </span>
              </TableCell>
              <TableCell heading="Most characters">
                <span className="skill-budget-number">
                  <span data-testid={`skill-budget-chars-${one.key}`}>{one.chars_said}</span>
                  <Button
                    size="sm"
                    variant="ghost"
                    className="skill-budget-press"
                    aria-label={`Reset to default, ${one.program}'s most characters`}
                    isDisabled={off || one.chars_here === null}
                    tooltip={unavailable(one.chars_here)}
                    onPress={() =>
                      reset({
                        program: one,
                        skills: one.skills_here,
                        chars: null,
                        said: resetSaid(one, "most characters"),
                      })
                    }
                  >
                    Reset to default
                  </Button>
                </span>
              </TableCell>
              <TableCell heading="Change">
                <Button
                  size="sm"
                  className="skill-budget-press skill-budget-change-press"
                  aria-label={`Change ${one.program}'s skill budget`}
                  isDisabled={off}
                  tooltip={readOnly}
                  onPress={() => setChanging(one.key)}
                >
                  Change…
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
      {refused !== undefined && (
        <p className="honest" role="alert" data-testid="skill-budget-refused">
          {refused}
        </p>
      )}
      {open !== undefined && (
        <SkillBudgetChange
          plane={plane}
          program={open}
          level={level}
          extensionRouter={extensionRouter}
          onClose={() => setChanging(undefined)}
          onWrite={onWrite}
        />
      )}
    </div>
  );
}

/**
 * **Change…: one program's budget at this level** (#5353). Each box holds this level's number, or
 * nothing for the number below it. As it is typed the dialog says what new chats would list and
 * hide, about how many tokens that costs per chat, what would go past what the program can hold,
 * and what stays listed below the pins. Those warn, and only a box that holds no number stops it.
 */
export function SkillBudgetChange({
  plane,
  program,
  level,
  extensionRouter,
  onClose,
  onWrite,
}: {
  plane: string;
  program: StudioBudget;
  level: BudgetLevel;
  extensionRouter: boolean;
  onClose: () => void;
  onWrite: (write: BudgetWrite, told: BudgetWriteTold) => void;
}) {
  const [skillsText, setSkillsText] = useState(program.skills_here?.toString() ?? "");
  const [charsText, setCharsText] = useState(program.chars_here?.toString() ?? "");
  const [worked, setPreview] = useState<SkillBudgetPreview>();
  const [problem, setProblem] = useState<string>();
  const [refused, setRefused] = useState<string>();
  const skills = budgetTyped(skillsText);
  const chars = budgetTyped(charsText);
  const valid = skills !== undefined && chars !== undefined;
  useEffect(() => {
    if (skills === undefined || chars === undefined) return;
    let live = true;
    const asked = setTimeout(() => {
      void commands
        .skillBudgetPreview(plane, level.workspace, level.persona, program.key, skills, chars)
        .then((answer) => {
          if (!live) return;
          if (answer.status === "error") setProblem(answer.error);
          else {
            setProblem(undefined);
            setPreview(answer.data);
          }
        })
        .catch((err: unknown) => live && setProblem(String(err)));
    }, TYPING_RESTS_MS);
    return () => {
      live = false;
      clearTimeout(asked);
    };
  }, [plane, level.workspace, level.persona, program.key, skills, chars]);
  // What a box that holds no number would do is nothing to say.
  const preview = valid && problem === undefined ? worked : undefined;
  const unchanged = skills === program.skills_here && chars === program.chars_here;
  const whose =
    level.workspace === null
      ? "For every chat in this project, until a workspace sets its own"
      : `For chats in ${level.workspace}`;
  return (
    <Dialog
      onClose={onClose}
      title={`Change ${program.program}'s skill budget`}
      subtitle={whose}
      description={`Past these numbers, the router moves what it can of a chat's skills into an index skill. Leave a box empty for the number below it.`}
      summary={BUDGET_APPLIES}
      main={{
        children: "Change budget",
        isDisabled: !valid || unchanged,
        onPress: () => {
          if (skills === undefined || chars === undefined) return;
          setRefused(undefined);
          onWrite(
            {
              program,
              skills,
              chars,
              said: `${program.program}'s skill budget is changed. ${BUDGET_APPLIES}`,
            },
            { done: onClose, refused: setRefused },
          );
        },
      }}
      data-testid="skill-budget-change"
    >
      <TextField
        label="Most skills in a chat's list"
        inputMode="numeric"
        autoFocus
        value={skillsText}
        onChange={setSkillsText}
        help={program.skills_said}
        validate={(typed) => (budgetTyped(typed) === undefined ? BUDGET_INVALID : undefined)}
      />
      <TextField
        label="Most characters in a chat's list"
        inputMode="numeric"
        value={charsText}
        onChange={setCharsText}
        help={program.chars_said}
        validate={(typed) => (budgetTyped(typed) === undefined ? BUDGET_INVALID : undefined)}
      />
      <div className="skill-budget-cost" role="status" data-testid="skill-budget-cost">
        {valid && preview === undefined && problem === undefined && (
          <p className="honest">Working out what this would do…</p>
        )}
        {problem !== undefined && <p className="honest">{problem}</p>}
        {preview !== undefined &&
          budgetCostSaid(preview).map((line) => (
            <p key={line} className="honest">
              {line}
            </p>
          ))}
        {preview?.past_limit && (
          <p className="honest" data-testid="skill-budget-past-limit">
            {preview.past_limit}
          </p>
        )}
        {preview?.over && (
          <p className="honest" data-testid="skill-budget-over">
            {preview.over}
          </p>
        )}
        {extensionRouter && (
          <p className="honest" data-testid="skill-budget-by-the-built-in-router">
            These chats have a router extension, which is asked only at a chat's start. This is what
            the built-in router would do.
          </p>
        )}
      </div>
      {refused !== undefined && (
        <p className="honest" role="alert" data-testid="skill-budget-change-refused">
          {refused}
        </p>
      )}
    </Dialog>
  );
}
