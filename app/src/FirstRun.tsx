// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type FirstRunFound, type TemplateChoice, type TemplateRow } from "./bindings";

/** The radio values that are not a template's id. Ids are lower-case words, so neither clashes. */
const FITS = ":fits";
const NONE = ":none";

function choiceOf(value: string): TemplateChoice {
  if (value === FITS) return { kind: "fits" };
  if (value === NONE) return { kind: "no-template" };
  return { kind: "named", id: value };
}
import { type ForgeAsk, ForgeQuestion } from "./ForgeQuestion";
import { DoorPage } from "./DoorPage";
import { useNewerTrouble } from "./pickTrouble";
import { harnessSays } from "./harnessSays";
import { Choice, Field, SettingActions, SettingRow } from "./settings/components";

/**
 * **Create a project: the page behind the welcome's first door** (#1812; FR-4 #603, FR-17).
 *
 * It was the first run, drawn in the opener's place only on a machine that remembered nothing;
 * now it is one press from the welcome on every machine. **One question, and it is not where
 * the project goes** (W10, as amended by Q1/Q2 for the forge-hosted create of #1819, which will
 * ask): purlis keeps a project on this machine, in its own directory and with no remote, and the
 * person is asked only for the repo to work on. That repo becomes a workspace named after it,
 * and the first chat starts in its clone. The copy is ADR 0072's: a code repo is a "repo", the
 * project is a "project". A project in a folder of the person's choosing is New project…, one
 * press away at the bottom of the page.
 *
 * **What the machine has is shown, never asked about**: which harnesses are installed and signed
 * in. A harness that is not signed in still starts, and asks for its own login in its own first
 * screen, which is where that question belongs. The forge tools' sign-ins moved to the welcome's
 * own sign-in entry (`ForgeSignIn.tsx`).
 *
 * **The project template is chosen here, and already chosen** (FR-17). It starts on the one that
 * fits the repo, which the core works out from the files at the repo's top level, so somebody
 * who does not look at it still gets one, and picking it costs no extra step. A template adds
 * personas, a review checklist and the commands every harness asks about first; it writes
 * nothing into the repo.
 *
 * The path box is there for the welcome's reason: a native folder dialog cannot be driven by
 * the scenario tests, and somebody who knows the path types it faster than they click.
 */
export function FirstRun({
  onOpenRepo,
  onBack,
  onNewProject,
  opening,
  trouble,
  forgeAsk,
}: {
  /** Asks the core to open this repo into the local project, laid out from `template`. */
  onOpenRepo: (path: string, template: TemplateChoice) => void;
  /** Comes back to the welcome. */
  onBack: () => void;
  /** Opens the New project dialog, for a project in a folder of its own. */
  onNewProject: () => void;
  /** Whether the core is cloning it right now, so it is not asked twice. */
  opening: boolean;
  /** Why the last attempt opened nothing — the core's words, all of them. */
  trouble?: string;
  /** Which forge the repo's repos are on, when its remote did not say (#839). */
  forgeAsk?: ForgeAsk;
}) {
  const [found, setFound] = useState<FirstRunFound>();
  /** Why purlis could not look at this machine, in the core's words: said, not swallowed. */
  const [unread, setUnread] = useState<string>();
  const [typed, setTyped] = useState("");
  const [template, setTemplate] = useState(FITS);
  // The template that fits a typed path, by id, with the path it was asked about: `null` when
  // none fits. A folder picked in the dialog opens at once, so this is the typed path's alone,
  // and an answer about a path no longer typed is not shown.
  const [fitting, setFitting] = useState<{ path: string; id: string | null }>();
  const path = typed.trim();
  const fits = fitting !== undefined && fitting.path === path ? fitting.id : undefined;

  useEffect(() => {
    if (path === "") return;
    let gone = false;
    void commands
      .templateThatFits(path)
      .then((answer) => {
        if (!gone && answer.status === "ok") setFitting({ path, id: answer.data });
      })
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [path]);

  useEffect(() => {
    let gone = false;
    void commands
      .firstRunFound()
      .then((answer) => {
        if (gone) return;
        if (answer.status === "ok") setFound(answer.data);
        else setUnread(answer.error);
      })
      // A machine purlis could not look at still opens a repo, and says it could not look
      // (#1719), where "On this machine" would otherwise just be absent.
      .catch((err: unknown) => {
        if (!gone) setUnread(String(err));
      });
    return () => {
      gone = true;
    };
  }, []);

  // A cancelled dialog is null and says nothing. One that could not open is said where a refused
  // open is (#1291), until any open starts or a newer refusal comes.
  const { said, pickFailed, started } = useNewerTrouble(trouble);
  const pick = useCallback(() => {
    started();
    void commands
      .pickProject()
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answer) => {
        if (answer.status === "error") pickFailed(answer.error);
        else if (answer.data) onOpenRepo(answer.data, choiceOf(template));
      });
  }, [onOpenRepo, template, pickFailed, started]);

  return (
    <DoorPage title="Create a project" className="first-run" onBack={onBack}>
      <p className="welcome-lede">
        A project is where purlis keeps your workspaces, personas and memory. purlis keeps this one
        on this machine and opens your repo in a workspace of its own. Nothing is written into your
        repo.
      </p>

      {/* **The template first** (#1719): both acts below open the repo with it, so it is seen
          before either is pressed. Until the machine is read, a line stands where it will be,
          so the acts do not jump when it arrives. */}
      {found === undefined && unread === undefined && (
        <p className="pending" role="status" aria-busy="true">
          Reading what this machine has…
        </p>
      )}
      {unread !== undefined && (
        <p className="came-back" role="status">
          purlis could not read what this machine has: {unread}
        </p>
      )}
      {found && found.templates.length > 0 && (
        <SettingRow
          label="Project template"
          help={
            "Personas, a review checklist and the commands purlis asks you about before a chat " +
            "runs them, for the stack you work in. Nothing is written into your repo."
          }
          grouped
          control={(ids) => (
            <Choice
              ids={ids}
              kind="radio"
              options={[
                { value: FITS, label: "Fits the repo", says: fitsSays(fits, found.templates) },
                ...found.templates.map((one) => ({
                  value: one.id,
                  label: one.title,
                  says: one.summary,
                })),
                { value: NONE, label: "None", says: "Only the steward persona." },
              ]}
              value={template}
              onValueChange={setTemplate}
              disabled={opening}
            />
          )}
        />
      )}

      {/* One leading row of the two ways in (#1719), and the path form's own below it.
          `tabIndex={0}` on every button, per `docs/ui-primitives.md` (charter-app#189). */}
      <SettingActions>
        <button type="button" tabIndex={0} disabled={opening} onClick={pick}>
          Open a repo…
        </button>
      </SettingActions>

      {/* The path and the template are rows of the settings set (DS-3d, #1176). */}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          started();
          if (typed.trim() && !opening) onOpenRepo(typed.trim(), choiceOf(template));
        }}
      >
        <SettingRow
          label="Or type the repo's path"
          help="A repo's folder on this machine."
          control={(ids) => (
            <Field
              ids={ids}
              kind="text"
              value={typed}
              placeholder="/path/to/repo"
              onChange={setTyped}
            />
          )}
        />
        <SettingActions>
          {/* The same act as New project's, in the same words (`docs/ui-copy.md`, #630). */}
          <button type="submit" tabIndex={0} disabled={!typed.trim() || opening}>
            Open repo
          </button>
        </SettingActions>
      </form>

      {opening && (
        <p className="came-back" role="status">
          Copying your repo into its workspace…
        </p>
      )}

      {forgeAsk && <ForgeQuestion ask={forgeAsk} />}

      {/* Verbatim: the sentence names the path and what was wrong with it. */}
      {said && (
        <p className="trouble said-in-full" role="alert">
          {said}
        </p>
      )}

      {found && (
        <>
          <h2 id="first-run-found">On this machine</h2>
          {found.harnesses.every((row) => !row.installed) && (
            <p className="came-back">
              No harness is installed on this machine. Once your repo is open, purlis lists each one
              with its own installer, which runs in a shell tab when you press Install.
            </p>
          )}
          <ul className="first-run-found" aria-labelledby="first-run-found">
            {found.harnesses.map((row) => (
              <li key={row.name}>
                <span className="tab-name">{row.title}</span>: {harnessSays(row)}
              </li>
            ))}
          </ul>
        </>
      )}

      {/* The other way to make one, in the catalogue's words for it (`project.create`). */}
      <p className="welcome-elsewhere">
        To keep the project in a folder you choose, use{" "}
        <button type="button" tabIndex={0} className="welcome-link" onClick={onNewProject}>
          New project…
        </button>
      </p>
    </DoorPage>
  );
}

/** What "Fits the repo" says it will pick: the template, once there is a path to look at. */
function fitsSays(fits: string | null | undefined, templates: TemplateRow[]): string {
  if (fits === undefined) return "purlis picks by the files at the repo's top level.";
  if (fits === null) return "None fits this repo, so it opens with no template.";
  const title = templates.find((one) => one.id === fits)?.title ?? fits;
  return `${title}, by the files at the repo's top level.`;
}
