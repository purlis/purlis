// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { Check, LoaderCircle, Stethoscope, TriangleAlert, X, type LucideIcon } from "lucide-react";
import { Notice } from "./Notice";
import { landSettingsFocus } from "./settings/entering";
import {
  FIXES_WITH_A_FORM,
  GitIdentityForm,
  type IdentityNow,
  type IdentityRefused,
} from "./GitIdentityForm";
import {
  commands,
  type DoctorFix,
  type DoctorFixed,
  type DoctorReport,
  type DoctorRow,
  type MoveIntoReposPlan,
  type PlaneId,
} from "./bindings";
import { MOVE_INTO_REPOS, MoveIntoReposConfirm, shownMoving } from "./MoveIntoReposConfirm";
import { said, useSwitches } from "./switchClock";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";
import { StatusItem, StatusVerdict } from "./ui/status/StatusItem";

/**
 * **`charter doctor`, from inside the window** — the last of ADR 0038's named gaps.
 *
 * The case for it is one incident, and it decides the shape. A charter launched from Finder
 * could not find `claude`, because macOS hands a GUI app a four-directory `PATH`
 * (charter-app#134). The doctor was ported and would have said so — but it answers for the
 * process that runs it, and the only way to run it was a terminal, whose shell has the
 * operator's whole `PATH` and so cannot reproduce the thing being diagnosed. So the doctor
 * runs **in the app's process** (`app/src-tauri/src/doctor.rs`) and is drawn here.
 *
 * # Where it lives: the status line, as one button
 *
 * The status line is the frame, not a region (`StatusLine.tsx` argues it): always drawn, never
 * put away. That is the property a health indicator needs — a doctor in a region can be hidden
 * by the operator and then report nothing, which is the "absent answer read as health" charter
 * ADR 0013 forbids. And it is one button, not a row of counts, because what the line can carry
 * is a verdict; the rows themselves are a list, and a list is a dialog.
 *
 * # The verdict counts only what this build checked
 *
 * About twenty of the doctor's rows are WARNs that say *not checked (…)*, for checks this build
 * does not run (planned in OB-8, #994, and FG-2, #802). They are
 * drawn in the dialog — a doctor that dropped them would read as those problems being fixed —
 * but they are never COUNTED on the line: a count that includes them never goes below twenty,
 * and the one real warning among them is furniture on its first day. The core tells the two
 * apart (`Row::deferred`), so this does not guess from the wording.
 *
 * # One row is the app's own
 *
 * `charter doctor` answers about the plane. One question is about what THIS APP does when it
 * starts a chat — whether the chats it opens can record what a turn cost — and no CLI can
 * answer it, so the core hands it over beside the table (`DoctorReport.app_rows`). It is
 * counted with the rest, because it is a check this build runs and a warning an operator can
 * act on; the dialog draws it under its own heading so nobody goes looking for it in
 * `charter doctor`'s output.
 *
 * And the footer's rule holds: **zero draws nothing.** A clean doctor is the word `Doctor` with
 * no number — never a green tick, because about twenty checks did not run and a tick over them
 * would be the claim ADR 0013 exists to refuse.
 *
 * # Two depths
 *
 * The window runs the **preflight** when a project opens — what every session start already
 * runs, so asking it unprompted costs nothing new. Opening the dialog runs the **full**
 * doctor, which also probes each harness profile: that RUNS the harness, and the core says
 * only a doctor a person asked for may (`doctor/profiles.rs`). Opening the dialog is the
 * asking. The dialog says which of the two it is showing.
 *
 * # Fix
 *
 * A row charter can fix itself carries a fix id (`DoctorRow.fix`, FX-1), and the dialog draws a
 * Fix button for it. The button calls the same core entry point `charter doctor --fix <id>`
 * does, says what the fix changed or why it was refused, and then checks again, so the row is
 * redrawn from what is true after the fix rather than assumed fixed.
 *
 * A fix that takes input (`git-identity`, FX-3) opens its form under the row instead, and is
 * applied when the form is sent: see `GitIdentityForm.tsx`.
 *
 * A finding whose fix id is in `FINDINGS_AS_NOTICES` also stands as a Notice in the Inbox
 * (`DoctorNotices`, #1250), and the Notice's Fix does what the row's does: the same form, the
 * same check again, so the Notice goes when the doctor no longer finds it.
 */

/** What the window knows about the doctor for one project. */
export type DoctorState = {
  /** The last report that came back. Kept while a newer one is being asked for, so the
   *  dialog does not blank while it checks again. */
  report?: DoctorReport;
  /** Whether an ask is on its way. */
  running: boolean;
  /** Why the last ask did not come back with a report, in the core's words. */
  trouble?: string;
  /** Ask again — `full` probes the harness profiles. */
  run: (full: boolean) => void;
  /** Apply the fix a row carries, by its id, then check again (the full doctor: the dialog
   *  is open). Absent where nothing can apply one. */
  fix?: (id: DoctorFix) => void;
  /** Apply each of these fixes in turn, then check again once (#5645): the doctor's one Notice
   *  line's Fix all. Stops at the first the core could not be asked to apply. */
  fixAll?: (ids: readonly DoctorFix[]) => void;
  /** Apply `git-identity` with the form's name and email, then check again. Resolves with
   *  each field's refusal, or `undefined` once the fix ran or was refused as a whole. */
  fixIdentity?: (name: string, email: string) => Promise<IdentityRefused | undefined>;
  /** git's global identity as it stands, for the form to lock what is set. */
  identityNow?: () => Promise<IdentityNow>;
  /** What `move-into-repos` would do, for its confirmation, or why it could not be read: the
   *  confirmation then says so and the fix is not applied (#5374). */
  movePlan?: () => Promise<MovePlanRead>;
  /** Apply `move-into-repos` as its confirmation showed it, then check again: a workspace
   *  that has other repos to move now is refused with nothing moved (#5569). */
  moveIntoRepos?: (shown: MoveIntoReposPlan[]) => void;
  /** The fix on its way, by its id. */
  fixing?: string;
  /** What the last fix came to: its lines, or why it was refused. */
  fixed?: DoctorFixed;
  /** Forget what the last fix said, and drop the answer of one still on its way: the dialog
   *  closed, and what it said belongs to the dialog that showed it. */
  forget?: () => void;
};

/**
 * The doctor for one project: the preflight once when it opens, and whatever is asked after.
 *
 * **The newest ask wins**, and that is a rule rather than a race: the preflight asked at open
 * and the full doctor asked a moment later by the operator opening the dialog can come back in
 * either order, and the full one must not be overwritten by the preflight that started first
 * and finished second.
 */
export function useDoctor(plane: PlaneId): DoctorState {
  const [report, setReport] = useState<DoctorReport>();
  // `true` from the first render, because the preflight is asked the moment the project
  // opens — and a flag the effect set would be a render the window does not need.
  const [running, setRunning] = useState(true);
  const [trouble, setTrouble] = useState<string>();
  const newest = useRef(0);

  /** Asks, and lands the answer only if nothing newer was asked since. */
  const ask = useCallback(
    (full: boolean) => {
      const mine = ++newest.current;
      void commands
        .planeDoctor(plane, full)
        .then((answer) => {
          if (mine !== newest.current) return;
          if (answer.status === "ok") {
            // Something that is not a report — nothing at all, or a test's catch-all `[]` — is
            // not drawn: a verdict made out of it would be a verdict made of nothing, and a
            // missing `rows` would take the whole window down with it.
            if (Array.isArray(answer.data?.rows)) {
              setReport(answer.data);
              setTrouble(undefined);
            }
          } else {
            setTrouble(answer.error);
          }
        })
        .catch((err: unknown) => {
          if (mine === newest.current) setTrouble(String(err));
        })
        .finally(() => {
          if (mine === newest.current) setRunning(false);
        });
    },
    [plane],
  );

  const run = useCallback(
    (full: boolean) => {
      setRunning(true);
      ask(full);
    },
    [ask],
  );

  useEffect(() => {
    ask(false);
  }, [ask]);

  // **A fix is about the plane it was asked of.** Kept with that plane, so a window that
  // moves to another project draws none of it, and numbered as `ask` numbers its asks, so an
  // answer that lands after the project changed (or the dialog closed) is dropped, not drawn.
  const [lastFix, setLastFix] = useState<{
    plane: PlaneId;
    fixing?: DoctorFix;
    fixed?: DoctorFixed;
  }>();
  const newestFix = useRef(0);
  useEffect(
    () => () => {
      newestFix.current += 1;
    },
    [plane],
  );
  const sendFix = useCallback(
    (id: DoctorFix, send: () => ReturnType<typeof commands.planeDoctorFix>) => {
      const mine = ++newestFix.current;
      setLastFix({ plane, fixing: id });
      const landed = (fixed: DoctorFixed) => {
        if (mine !== newestFix.current) return;
        setLastFix({ plane, fixed });
        // Checked again whatever came of it: a fix that half-ran changed something too.
        run(true);
      };
      void send()
        .then((answer) =>
          landed(
            answer.status === "ok"
              ? answer.data
              : { fix: id, refused: answer.error, said: [], complete: false },
          ),
        )
        .catch((err: unknown) =>
          landed({ fix: id, refused: String(err), said: [], complete: false }),
        );
    },
    [plane, run],
  );
  const fix = useCallback(
    (id: DoctorFix) => sendFix(id, () => commands.planeDoctorFix(plane, id)),
    [plane, sendFix],
  );
  const fixAll = useCallback(
    (ids: readonly DoctorFix[]) => {
      const [first] = ids;
      if (first === undefined) return;
      sendFix(first, async () => {
        let answer = await commands.planeDoctorFix(plane, first);
        for (const id of ids.slice(1)) {
          if (answer.status !== "ok") break;
          answer = await commands.planeDoctorFix(plane, id);
        }
        return answer;
      });
    },
    [plane, sendFix],
  );
  const moveIntoRepos = useCallback(
    (shown: MoveIntoReposPlan[]) =>
      sendFix(MOVE_INTO_REPOS, () => commands.planeDoctorMoveIntoRepos(plane, shown)),
    [plane, sendFix],
  );
  const fixIdentity = useCallback(
    async (name: string, email: string): Promise<IdentityRefused | undefined> => {
      const mine = ++newestFix.current;
      setLastFix({ plane, fixing: "git-identity" });
      const sent = await sendIdentity(plane, name, email);
      if ("refused" in sent) {
        // Refused field by field: nothing was written, and nothing is said over the rows.
        if (mine === newestFix.current) setLastFix(undefined);
        return sent.refused;
      }
      if (mine === newestFix.current) {
        setLastFix({ plane, fixed: sent.fixed });
        run(true);
      }
      return undefined;
    },
    [plane, run],
  );
  const identityNow = useCallback(() => identityNowOf(plane), [plane]);
  const movePlan = useCallback(() => movePlanOf(plane), [plane]);
  const forget = useCallback(() => {
    newestFix.current += 1;
    setLastFix(undefined);
  }, []);
  const ours = lastFix?.plane === plane ? lastFix : undefined;

  return {
    report,
    running,
    trouble,
    run,
    fix,
    fixAll,
    fixIdentity,
    identityNow,
    movePlan,
    moveIntoRepos,
    fixing: ours?.fixing,
    fixed: ours?.fixed,
    forget,
  };
}

/**
 * **The git identity fix, sent with the form's name and email** (FX-3): what the core wrote, or
 * each field's refusal when it wrote nothing. A failure to ask at all is a fix refused as a
 * whole. Shared by the doctor's own form and an Inbox alert's (#1301).
 */
export async function sendIdentity(
  plane: PlaneId,
  name: string,
  email: string,
): Promise<{ fixed: DoctorFixed } | { refused: IdentityRefused }> {
  const whole = (why: string): { fixed: DoctorFixed } => ({
    fixed: { fix: "git-identity", refused: why, said: [], complete: false },
  });
  try {
    const answer = await commands.planeDoctorFixIdentity(plane, name, email);
    if (answer.status !== "ok") return whole(answer.error);
    if (answer.data.kind === "fixed") return { fixed: answer.data.fixed };
    return { refused: { name: answer.data.name, email: answer.data.email } };
  } catch (err: unknown) {
    return whole(String(err));
  }
}

/** git's global identity as it stands, for the form to lock what is set. */
export async function identityNowOf(plane: PlaneId): Promise<IdentityNow> {
  // Nothing read is nothing locked: the core reads it again before it writes anyway.
  const none = { name: "", email: "" };
  try {
    const answer = await commands.planeDoctorIdentity(plane);
    return answer.status === "ok" && answer.data ? answer.data : none;
  } catch {
    return none;
  }
}

/** What `move-into-repos` would do, or why that could not be read. */
export type MovePlanRead = { plans: MoveIntoReposPlan[] } | { failed: string };

/** What `move-into-repos` would do in `plane`, or why it could not be asked. A failure is never
 *  a reason to move without asking (#5374). */
export async function movePlanOf(plane: PlaneId): Promise<MovePlanRead> {
  try {
    const answer = await commands.planeDoctorMoveIntoReposPlan(plane);
    if (answer.status !== "ok") return { failed: answer.error };
    return Array.isArray(answer.data)
      ? { plans: answer.data }
      : { failed: "the core gave no answer" };
  } catch (err: unknown) {
    return { failed: err instanceof Error ? err.message : String(err) };
  }
}

/** Every row the verdict counts: the table's, and the app's own beside it. */
export function everyRow(report: DoctorReport): DoctorRow[] {
  return [...report.rows, ...(report.app_rows ?? [])];
}

/** The rows sorted into what the line and the dialog say about them. */
export function sorted(rows: readonly DoctorRow[]) {
  return {
    blockers: rows.filter((row) => row.status === "fail"),
    // A deferred row is a WARN too, and is not counted with the real ones: see the module doc.
    warnings: rows.filter((row) => row.status === "warn" && row.checked),
    passed: rows.filter((row) => row.status === "ok"),
    unchecked: rows.filter((row) => !row.checked),
  };
}

/** `2 blockers`, `1 warning` — or nothing, for the footer's reason. */
function counted(n: number, one: string): string | undefined {
  if (n === 0) return undefined;
  return n === 1 ? `1 ${one}` : `${n} ${one}s`;
}

/** What the button on the status line says, and what it says to a screen reader. */
export function onTheLine(doctor: DoctorState): { said?: string; tone: string; label: string } {
  const { report, running, trouble } = doctor;
  if (report === undefined) {
    if (trouble !== undefined)
      return {
        tone: "unknown",
        label: `Doctor — could not run: ${trouble}`,
      };
    return { tone: "unknown", label: running ? "Doctor — checking" : "Doctor" };
  }
  const { blockers, warnings, unchecked } = sorted(everyRow(report));
  const said = counted(blockers.length, "blocker") ?? counted(warnings.length, "warning");
  const tone = blockers.length > 0 ? "fail" : warnings.length > 0 ? "warn" : "quiet";
  const label =
    `Doctor: ${said ?? "nothing wrong among the checks this build runs"}` +
    (unchecked.length > 0 ? `; ${unchecked.length} not checked by this build` : "");
  return { said, tone, label };
}

/** The mark each verdict is drawn with: the icon set's for what `purlis doctor`'s own table
 *  writes as ✓, ! and ✗, so the two read alike (DS-4, #627). Decorative: the heading a row is
 *  under says its verdict in words. */
const MARK: Record<DoctorRow["status"], LucideIcon> = { ok: Check, warn: TriangleAlert, fail: X };

/** What a row's Fix button needs: the doctor's `fix`, and whether one may be pressed now. */
export type Fixer = {
  apply: (id: DoctorFix) => void;
  busy: boolean;
  /** The git identity form's submit; absent where no form can be sent. */
  identity?: (name: string, email: string) => Promise<IdentityRefused | undefined>;
  /** What the identity is now, read when its form opens. */
  identityNow?: () => Promise<IdentityNow>;
  /** What `move-into-repos` would do, read when its confirmation opens (#5374). */
  movePlan?: () => Promise<MovePlanRead>;
  /** `move-into-repos`, sent with what its confirmation showed (#5569). */
  moveIntoRepos?: (shown: MoveIntoReposPlan[]) => void;
};

/**
 * **A fix's form, wherever its fix is pressed** (FX-3, #1250): the Doctor row's Fix and a
 * Notice carrying the same fix id open it alike, and so does an Alerts-drawer row whose way out
 * is that fix (#1301). For a fix in {@link FIXES_WITH_A_FORM},
 * `press` opens the form (once what is set has been read, which is what it locks) or closes
 * it; for any other, it applies the fix. `form` is the form while it is open, and it closes
 * itself once the fix was applied or refused as a whole. With no `fix` (a place that draws one
 * only sometimes, and asks for this hook every time), it is offered nowhere and presses nothing.
 */
export function useFixForm(fix: DoctorFix | undefined, fixer: Fixer) {
  // The form is open once what is set has been read: `current` is what it locks.
  const [current, setCurrent] = useState<IdentityNow>();
  const id = useId();
  const withForm = fix !== undefined && FIXES_WITH_A_FORM.has(fix);
  const open = current !== undefined;
  // `move-into-repos` asks first, naming what it moves and what it costs (#5374).
  const [plans, setPlans] = useState<MovePlanRead>();
  const press = () => {
    if (fix === undefined) return;
    if (fix === MOVE_INTO_REPOS && fixer.movePlan) {
      void fixer.movePlan().then(setPlans);
    } else if (!withForm) fixer.apply(fix);
    else if (open) setCurrent(undefined);
    else void (fixer.identityNow?.() ?? Promise.resolve({ name: "", email: "" })).then(setCurrent);
  };
  const form = withForm && current !== undefined && fixer.identity && (
    <GitIdentityForm
      id={id}
      current={current}
      busy={fixer.busy}
      onCancel={() => setCurrent(undefined)}
      submit={async (name, email) => {
        const refused = await fixer.identity?.(name, email);
        if (refused === undefined) setCurrent(undefined);
        return refused;
      }}
    />
  );
  const confirm = plans !== undefined && (
    <MoveIntoReposConfirm
      {...plans}
      onCancel={() => setPlans(undefined)}
      onMove={() => {
        setPlans(undefined);
        // Only what the confirmation showed moves: a repo added since refuses the move, and a
        // workspace it left as it is stays so (#5569).
        if ("plans" in plans) fixer.moveIntoRepos?.(shownMoving(plans.plans));
      }}
    />
  );
  return {
    /** Whether this fix can be pressed here: one with a form needs somewhere to send it, and
     *  `move-into-repos` a way to ask what it would do. */
    offered:
      fix === undefined
        ? false
        : fix === MOVE_INTO_REPOS
          ? fixer.movePlan !== undefined && fixer.moveIntoRepos !== undefined
          : !withForm || fixer.identity !== undefined,
    press,
    /** The button's state, for a fix with a form. */
    opens: withForm ? { id, open } : undefined,
    form: form || confirm || undefined,
  };
}

/** A row's Fix button, and the form under it for a fix that takes input (FX-3). */
function FixButton({ row, fixer }: { row: DoctorRow & { fix: DoctorFix }; fixer: Fixer }) {
  const { offered, press, opens, form } = useFixForm(row.fix, fixer);
  if (!offered) return null;
  return (
    <>
      <Button
        size="sm"
        aria-label={`Fix ${row.name}`}
        aria-expanded={opens?.open}
        aria-controls={opens?.open ? opens.id : undefined}
        tooltip={`purlis doctor --fix ${row.fix}`}
        isDisabled={fixer.busy}
        onPress={press}
        className="doctor-fix"
      >
        Fix
      </Button>
      {form}
    </>
  );
}

/**
 * **The fix ids whose doctor finding stands as a Notice in the Inbox** (#1250), and not
 * only as a row of the Doctor dialog.
 *
 * **The rule** (D-1301-1): a finding stands here only where, left alone, it silently loses work
 * or blocks every chat, and its fix is one press. Everything else is the dialog's, because a
 * band of rows on every open would be noise. One finding stands by ruling instead (Q67, #1990):
 * `persona-agents` blocks nothing, and is here because removing the files is the person's chore,
 * which a line in every chat's briefing used to carry.
 *
 * - `git-identity` (`git identity`): without it every commit purlis makes (memory, notes,
 *   tallies) silently never happens. Its fix is a form the Notice opens where it stands.
 * - `memory-optimize` (`memory indexes`): a memory its `MEMORY.md` does not list is never read
 *   by a chat, so what was learnt is silently lost. The fix only appends the missing links.
 * - `persona-agents` (`personas`, #1990, Q67): the sub-agent files purlis generated for a
 *   persona, still in the project. Removing them is the person's chore, so it stands here once
 *   instead of a line in every chat's briefing. It runs only by its name, and the Notice's Fix
 *   names it as the dialog's does; it makes no commit, so the person reviews what it changed
 *   at the next save.
 *
 * And why each of the others stays in the dialog:
 * - `discover`, `rename-plane`, `rename-local` and `handoff-rule` run only by their name
 *   (`by_name_only` in core doctor/fix.rs): the network, this machine's folders, or committed
 *   files every teammate pulls. One press in the Inbox is not how those are asked.
 * - `plugin-install` is about the chats started outside the app; the app arms its own either
 *   way, and the doctor the window runs at open reports the row as fine.
 * - `reinit` creates a missing baseline folder, which blocks nothing in the meantime.
 * - `local-ignore` is found only by the doctor the dialog runs: the one at open does not ask
 *   git, so its report would let the Notice's dismissal go while the finding stands.
 * - `workspace-reinit` (`workspace layout`, #1289; D-1301-2): a workspace behind the layout
 *   still opens and works, so it blocks nothing, and the Inbox already stands it as its `reinit`
 *   alert with the same fix. A Notice as well would say one finding in two standing
 *   places.
 *
 * Each by the name of the doctor row that finds it: a clean row carries no fix id, so the name
 * is how a report says the finding has gone (and a dismissal of it is let go).
 */
export const FINDINGS_AS_NOTICES: ReadonlyMap<DoctorFix, string> = new Map<DoctorFix, string>([
  ["git-identity", "git identity"],
  ["memory-optimize", "memory indexes"],
  ["persona-agents", "personas"],
]);

/** The family of cause a doctor finding's Notice is (`doctor-finding:<fix id>`). */
export const DOCTOR_FINDING = "doctor-finding";

/** The cause the line dismisses a finding by (`doctor-finding:<…>`): a finding of
 *  {@link FINDINGS_AS_NOTICES} by its fix id, as it stood as a Notice of its own before #5645, so
 *  what was dismissed then stays so; any other by its row's name and its fix, if it has one, so
 *  two findings of one row are told apart. */
function causeOf(row: DoctorRow): string {
  if (row.fix !== null && FINDINGS_AS_NOTICES.get(row.fix) === row.name)
    return `${DOCTOR_FINDING}:${row.fix}`;
  return `${DOCTOR_FINDING}:${row.name}${row.fix === null ? "" : `|${row.fix}`}`;
}

/** The name of the doctor row a finding's cause is about. */
function rowNamed(cause: string): string {
  const said = cause.slice(DOCTOR_FINDING.length + 1);
  return FINDINGS_AS_NOTICES.get(said as DoctorFix) ?? said.split("|", 1)[0];
}

/** **What the doctor found** (#1693): each row that failed, and each warning of a check this
 *  build runs (a row it does not run is no finding: `DoctorRow.checked`). */
const isFinding = (row: DoctorRow): boolean =>
  row.status === "fail" || (row.status === "warn" && row.checked);

/** The causes of the findings `report` holds, as the doctor's line dismisses each: what the
 *  first-open baseline puts away (#5648). */
export const findingCauses = (report: DoctorReport): string[] =>
  [...report.rows, ...report.app_rows].filter(isFinding).map(causeOf);

/** The cause of the doctor's one Notice line (#5645): of the `doctor-finding` family, so the
 *  status line's Notices count leaves it to the doctor's own item (`FROM_THE_DOCTOR`). */
export const DOCTOR_LINE = `${DOCTOR_FINDING}:*`;

/** What the line says: how many findings it lists. */
const findingsSaid = (count: number) => `Doctor: ${count} ${count === 1 ? "finding" : "findings"}`;

/**
 * **Every doctor finding, as one Notice line** (#5645, ADR 0119): "Doctor: N findings", in the
 * Notices view, from the doctor the window already runs. It opens to the list (Show findings),
 * each finding as the Doctor dialog's row says it. A finding of {@link FINDINGS_AS_NOTICES}
 * (D-1301-1) carries its Fix there, a form for a fix that takes input (#1250); **Fix all**
 * applies those that one press may, then checks again. Every other fix stays the dialog's.
 *
 * **Dismiss puts away every finding it lists, until each changes** (V91j, `dismissals.ts`):
 * every report the doctor answers settles the `doctor-finding` family, so a finding that was
 * mended lets its dismissal go and shows again if it comes back, and a new finding stands the
 * line again. A report that did not check a finding's row (git could not be asked), or does not
 * hold it, says nothing about it, and keeps its dismissal.
 */
export function DoctorNotices({
  doctor,
  dismissed,
  dismiss,
  settle,
}: {
  doctor: DoctorState;
  /** The causes dismissed in this project, and the ways to change them (`useDismissals`). */
  dismissed: ReadonlySet<string>;
  dismiss: (cause: string) => void;
  settle: (family: string, present: readonly string[]) => void;
}) {
  const { report, fix } = doctor;
  const [open, setOpen] = useState(false);
  const listId = useId();
  /** What is dismissed now, read when a report lands without the report waiting on it. */
  const dismissedNow = useRef(dismissed);
  useEffect(() => {
    dismissedNow.current = dismissed;
  }, [dismissed]);

  useEffect(() => {
    if (report === undefined) return;
    const rows = [...report.rows, ...report.app_rows];
    const standing = rows.filter(isFinding).map(causeOf);
    // A dismissed finding whose row this report did not check, or holds not at all, is unknown,
    // so it is not let go.
    const unknown = [...dismissedNow.current].filter((cause) => {
      if (!cause.startsWith(`${DOCTOR_FINDING}:`)) return false;
      const row = rows.find((one) => one.name === rowNamed(cause));
      return row === undefined || !row.checked;
    });
    settle(DOCTOR_FINDING, [...new Set([...standing, ...unknown])]);
  }, [report, settle]);

  if (fix === undefined || report === undefined) return null;
  const listed = [...report.rows, ...report.app_rows]
    .filter(isFinding)
    .filter((row) => !dismissed.has(causeOf(row)));
  if (listed.length === 0) return null;

  const fixer: Fixer = {
    apply: fix,
    busy: doctor.running || doctor.fixing !== undefined,
    identity: doctor.fixIdentity,
    identityNow: doctor.identityNow,
    movePlan: doctor.movePlan,
    moveIntoRepos: doctor.moveIntoRepos,
  };
  // Only a finding of D-1301-1 is fixed from here; the rest are the dialog's.
  const shown = listed.map((row) =>
    row.fix !== null && FINDINGS_AS_NOTICES.has(row.fix) ? row : { ...row, fix: null },
  );
  const onePress = shown.flatMap((row) =>
    row.fix !== null && !FIXES_WITH_A_FORM.has(row.fix) && row.fix !== MOVE_INTO_REPOS
      ? [row.fix]
      : [],
  );
  // What the last fix of a listed finding came to, when it did not mend it: the doctor checks
  // again either way, and a finding still standing says why.
  const fixed = shown.some((row) => row.fix !== null && row.fix === doctor.fixed?.fix)
    ? doctor.fixed
    : undefined;
  const why =
    fixed !== undefined && fixed.refused !== null
      ? `The fix was refused: ${fixed.refused}`
      : fixed && !fixed.complete
        ? fixed.said.join(" ")
        : undefined;
  const { fixAll } = doctor;
  return (
    <Notice
      cause={DOCTOR_LINE}
      tone={listed.some((row) => row.status === "fail") ? "trouble" : "news"}
      fixes={[
        {
          label: "Show findings",
          onPress: () => setOpen((was) => !was),
          opens: { id: listId, open },
        },
        ...(onePress.length > 0 && fixAll !== undefined
          ? [{ label: "Fix all", onPress: () => fixAll(onePress), busy: fixer.busy }]
          : []),
      ]}
      onDismiss={() => {
        for (const row of listed) dismiss(causeOf(row));
      }}
      under={
        open ? (
          <div id={listId}>
            <Rows rows={shown} fixer={fixer} label="Doctor findings" />
          </div>
        ) : undefined
      }
    >
      {findingsSaid(listed.length)}
      {why !== undefined && <p>{why}</p>}
    </Notice>
  );
}

/** A verdict's mark, from {@link MARK}. */
function Mark({ status }: { status: DoctorRow["status"] }) {
  const Icon = MARK[status];
  return <Icon />;
}

/**
 * The rows of one heading. A row whose fix is a setting names that setting's group
 * (`DoctorRow.settings`, SE-22), and is drawn with the way into it: `onOpenSettings`. A row
 * charter can fix itself (`DoctorRow.fix`, FX-1) is drawn with a Fix button: `fixer`.
 */
function Rows({
  rows,
  fixer,
  onOpenSettings,
  label,
}: {
  rows: readonly DoctorRow[];
  fixer?: Fixer;
  onOpenSettings?: (group: string) => void;
  /** The list's name, where nothing above it names it: the doctor's Notice line's (#5645). */
  label?: string;
}) {
  return (
    <ul className="doctor-rows" aria-label={label}>
      {rows.map((row) => (
        <li key={row.name} className={`doctor-row doctor-${row.status}`} data-row={row.name}>
          <span className="doctor-glyph" aria-hidden="true">
            <Mark status={row.status} />
          </span>
          <span className="doctor-name">{row.name}</span>
          <span className="doctor-detail">{row.detail}</span>
          {/* A green row's hint is carried and never drawn, as the table does. A deferred
              row's hint is the same sentence on every one of them, so it is said once, over
              the list, rather than twenty-four times inside it. */}
          {row.status !== "ok" && row.checked && row.hint && (
            <span className="doctor-hint">{row.hint}</span>
          )}
          {row.settings !== null && onOpenSettings && (
            <Button
              size="sm"
              onPress={() => onOpenSettings(row.settings as string)}
              className="doctor-settings"
            >
              Fix it in Settings
            </Button>
          )}
          {row.status !== "ok" && row.fix && fixer && (
            <FixButton row={{ ...row, fix: row.fix }} fixer={fixer} />
          )}
        </li>
      ))}
    </ul>
  );
}

/**
 * The status line's doctor item, and the dialog it opens. A status item (the set's small ghost
 * button, `StatusLine.tsx`): flat on the bar, its mark, its word, then its count.
 *
 * The dialog family's `Dialog` (`ui/dialog/Dialog.tsx`), on React Aria's modal, which is what makes
 * the list keyboard-reachable and the window behind it inert while it is up.
 */
export function Health({
  doctor,
  onOpenSettings,
  counted = true,
}: {
  doctor: DoctorState;
  /** Draws its count on the line: false while the Notices view lists the findings, whose icon
   *  on the right bar shows them (ADR 0121 decision 1, #6004). Its name says the count still. */
  counted?: boolean;
  /** Opens Settings at a group (`settings/links.ts`, SE-22): what a row naming a setting's
   *  "Fix it in Settings" does, once the dialog has closed. */
  onOpenSettings?: (group: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const settings =
    onOpenSettings &&
    ((group: string) => {
      setOpen(false);
      onOpenSettings(group);
    });
  // Where the keyboard goes when a fix answers: the button it was pressed on is gone once the
  // row is clean, and focus left on nothing falls back to the page.
  const fixedAt = useRef<HTMLElement>(null);
  useEffect(() => {
    if (doctor.fixed !== undefined) fixedAt.current?.focus();
  }, [doctor.fixed]);
  const { said, tone, label } = onTheLine(doctor);
  const { report, running, trouble, run, fixing, fixed } = doctor;
  const groups = report ? sorted(report.rows) : undefined;
  const fixer: Fixer | undefined = doctor.fix && {
    apply: doctor.fix,
    busy: running || fixing !== undefined,
    identity: doctor.fixIdentity,
    identityNow: doctor.identityNow,
    movePlan: doctor.movePlan,
    moveIntoRepos: doctor.moveIntoRepos,
  };
  const ours = report?.app_rows ?? [];

  return (
    <>
      <StatusItem
        data-testid="status-doctor"
        tone={tone === "unknown" ? "quiet" : undefined}
        aria-label={label}
        tooltip={label}
        aria-haspopup="dialog"
        onPress={() => {
          setOpen(true);
          // Opening it is the operator asking, which is the one thing that may probe a harness.
          run(true);
        }}
        icon={
          running ? (
            // It is running: this is the state the spin is for, *still happening*.
            <LoaderCircle aria-hidden="true" className="spinning" />
          ) : (
            <Stethoscope aria-hidden="true" />
          )
        }
      >
        Doctor
        {/* The count, in its verdict's colour and in words, so the colour is never the only
            thing that says it; nothing at all when nothing is wrong (qw357). */}
        {counted && said !== undefined && (
          <StatusVerdict tone={tone === "fail" ? "failed" : "waiting"}>{said}</StatusVerdict>
        )}
      </StatusItem>
      <Dialog
        isOpen={open}
        onClose={() => {
          setOpen(false);
          doctor.forget?.();
        }}
        title="Doctor"
        className="doctor"
        aria-describedby="doctor-depth"
        // A row's "Fix it in Settings" sent the keyboard into Settings, rather than back to the
        // doctor's button (#1206).
        afterClose={() => void landSettingsFocus()}
        cancel={{ children: "Close" }}
        main={{
          children: "Check again",
          variant: "secondary",
          isDisabled: running,
          onPress: () => run(true),
        }}
      >
        <p className="honest" id="doctor-depth">
          {/* Work in progress says what is being done (`docs/ui-copy.md`, #630). */}
          {report === undefined
            ? running
              ? "Running the doctor…"
              : "The doctor has not answered yet."
            : report.full
              ? "Every check, with each harness profile probed. It runs inside this app, so every answer is the app's own environment."
              : "The preflight every chat start runs. The harness profiles are not probed. It runs inside this app, so every answer is the app's own environment."}
          {report !== undefined && running && " Checking again…"}
        </p>
        {trouble !== undefined && (
          <p className="honest doctor-trouble" role="alert">
            The doctor could not run: {trouble}
          </p>
        )}
        {fixed !== undefined && (
          <section
            aria-label="Fix"
            className={`doctor-fixed${fixed.complete ? "" : " doctor-trouble"}`}
            role="status"
            ref={fixedAt}
            tabIndex={-1}
          >
            {fixed.refused !== null ? (
              <p className="honest">
                Fix {fixed.fix} was refused: {fixed.refused}
              </p>
            ) : (
              <ul className="doctor-fixed-lines">
                {fixed.said.map((line, i) => (
                  <li key={i}>{line}</li>
                ))}
              </ul>
            )}
          </section>
        )}
        {ours.length > 0 && (
          <section aria-label="This app">
            <h3 className="ui-text-heading">This app</h3>
            {/* Said rather than left to be noticed: an operator comparing this with
              `purlis doctor` in a terminal has to know why one row is not there. */}
            <p className="honest">
              What the chats this window starts can do. <code>purlis doctor</code> does not print
              these.
            </p>
            <Rows rows={ours} onOpenSettings={settings} />
          </section>
        )}
        {groups && (
          <>
            {groups.blockers.length > 0 && (
              <section aria-label="Blockers">
                <h3 className="ui-text-heading">Blockers</h3>
                <Rows rows={groups.blockers} fixer={fixer} onOpenSettings={settings} />
              </section>
            )}
            {groups.warnings.length > 0 && (
              <section aria-label="Warnings">
                <h3 className="ui-text-heading">Warnings</h3>
                <Rows rows={groups.warnings} fixer={fixer} onOpenSettings={settings} />
              </section>
            )}
            {groups.passed.length > 0 && (
              <section aria-label="Passed">
                <h3 className="ui-text-heading">Passed</h3>
                <Rows rows={groups.passed} fixer={fixer} onOpenSettings={settings} />
              </section>
            )}
            {groups.unchecked.length > 0 && (
              <details className="doctor-unchecked">
                <summary>Not checked by this build ({groups.unchecked.length})</summary>
                {/* The one hint every deferred row carries, once. */}
                <p className="doctor-hint">{groups.unchecked[0].hint}</p>
                <Rows rows={groups.unchecked} />
              </details>
            )}
          </>
        )}
        <SwitchesTimed />
        {report && (
          <p className="doctor-path">
            <span className="ui-text-meta">This app&apos;s PATH</span>{" "}
            {report.path === null ? (
              <span className="none">none — the app was started with no PATH at all</span>
            ) : (
              <code>{report.path}</code>
            )}
          </p>
        )}
      </Dialog>
    </>
  );
}

/**
 * **What the project switches in this window took** (`switchClock.ts`), for a person to read and
 * copy: a dialog's text is selectable. Folded away, as the rows this build does not check are,
 * and not drawn at all in a window that has only ever shown one project: with nothing to switch
 * between there is nothing to say.
 */
function SwitchesTimed() {
  const switches = useSwitches();
  if (switches.length < 2) return null;
  const name = (plane: string) => plane.split(/[\\/]/).filter(Boolean).at(-1) ?? plane;
  return (
    <details className="doctor-unchecked">
      <summary>Project switches in this window ({switches.length})</summary>
      <p className="doctor-hint">
        How long each took, newest last: to the window having drawn the project, to that being
        painted, and each command the core answers on the window&apos;s own thread, where nothing
        else is drawn while it runs.
      </p>
      <ul className="doctor-fixed-lines">
        {switches.map((one) => (
          <li key={`${one.at}:${one.to}`}>{said(one, name)}</li>
        ))}
      </ul>
    </details>
  );
}
