// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useCallback, useEffect, useState } from "react";
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
import { rememberedParent, rememberParent } from "./CreateOnForge";
import { DoorPage } from "./DoorPage";
import { useNewerTrouble } from "./pickTrouble";
import { Choice, Field, SettingActions, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Form } from "./ui/field/Field";

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
  onMake,
  onBack,
  opening,
  trouble,
  forgeAsk,
  where,
  elsewhere,
}: {
  /** Asks the core to open this repo into the local project, laid out from `template`. */
  onOpenRepo: (path: string, template: TemplateChoice) => void;
  /** Comes back to the welcome. */
  onBack: () => void;
  /** Makes a project with no repo in this folder, laid out from `template`, and opens it. */
  onMake: (folder: string, template: string | null) => void;
  /** Whether the core is cloning it right now, so it is not asked twice. */
  opening: boolean;
  /** Why the last attempt opened nothing — the core's words, all of them. */
  trouble?: string;
  /** Which forge the repo's repos are on, when its remote did not say (#839). */
  forgeAsk?: ForgeAsk;
  /** Where the project lives (#1819, ruling Q2): drawn first, under the heading. */
  where?: ReactNode;
  /** **A project on a forge** (#1819): given, it is drawn in place of the page's own body (a
   *  repo on this machine, in the project purlis keeps here), with the templates this purlis
   *  ships once the machine has been read. */
  elsewhere?: (templates: TemplateRow[] | undefined) => ReactNode;
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
      {where}
      {elsewhere ? (
        elsewhere(found?.templates)
      ) : (
        <>
          {/* Nothing but a name is needed: a project with no repo yet, on this Mac (#1810 story 8). */}
          <OnThisMac onMake={onMake} making={opening} templates={found?.templates} />

          <h2>Or start from a repo you already have</h2>

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
                template === FITS
                  ? fitsSays(fits, found.templates)
                  : "Personas, a review checklist and the commands purlis asks you about before a " +
                    "chat runs them, for the stack you work in. Nothing is written into your repo."
              }
              control={(ids) => (
                <Choice
                  ids={ids}
                  kind="select"
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
            <Button isDisabled={opening} onPress={pick}>
              Open a repo…
            </Button>
          </SettingActions>

          {/* The path and the template are rows of the settings set (DS-3d, #1176). */}
          <Form
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
              <Button type="submit" isDisabled={!typed.trim() || opening}>
                Open repo
              </Button>
            </SettingActions>
          </Form>

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
        </>
      )}
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

/**
 * **A new project on this Mac, from its name**: no account and no repo. Its folder follows the
 * name, under the place picked last, as a project made on a forge does; a name that cannot be
 * one says why. There is no repo to fit a template to, so it starts on None (#4599).
 */
function OnThisMac({
  onMake,
  making,
  templates,
}: {
  onMake: (folder: string, template: string | null) => void;
  making: boolean;
  /** The templates this purlis ships, once the machine has been read. */
  templates: readonly TemplateRow[] | undefined;
}) {
  const [name, setName] = useState("");
  const [template, setTemplate] = useState(NONE);
  const [parent, setParent] = useState(rememberedParent);
  const [answered, setAnswered] = useState<{ name: string; folder?: string; why?: string }>();
  const [unpicked, setUnpicked] = useState<string>();
  const typed = name.trim();
  const here = answered?.name === typed ? answered : undefined;

  useEffect(() => {
    if (!typed) return;
    let gone = false;
    void commands
      .newProjectFolder(typed, parent ?? null)
      .then((answer) => {
        if (gone) return;
        setAnswered(
          answer.status === "ok"
            ? { name: typed, folder: answer.data }
            : { name: typed, why: answer.error },
        );
      })
      .catch((err: unknown) => {
        if (!gone) setAnswered({ name: typed, why: String(err) });
      });
    return () => {
      gone = true;
    };
  }, [typed, parent]);

  const choose = useCallback(() => {
    setUnpicked(undefined);
    void commands
      .pickProject()
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
      .then((answer) => {
        if (answer.status === "error") setUnpicked(answer.error);
        else if (answer.data) {
          rememberParent(answer.data);
          setParent(answer.data);
        }
      });
  }, []);

  const folder = here?.folder;
  const ready = folder !== undefined && !making;
  return (
    <Form
      onSubmit={(event) => {
        event.preventDefault();
        if (ready) onMake(folder, template === NONE ? null : template);
      }}
    >
      <SettingRow
        label="Name"
        setting="create-project-name"
        help="Letters, digits, dots, dashes and underscores, like my-project."
        control={(ids) => (
          <Field ids={ids} kind="text" value={name} placeholder="my-project" onChange={setName} />
        )}
      />
      {here?.why && (
        <p className="trouble" role="alert">
          {here.why}
        </p>
      )}
      {/* Drawn with None alone until the machine is read, so Create project does not jump. */}
      {templates?.length !== 0 && (
        <SettingRow
          label="Template"
          setting="create-project-template"
          help={
            templates?.find((one) => one.id === template)?.summary ?? "Only the steward persona."
          }
          control={(ids) => (
            <Choice
              ids={ids}
              kind="select"
              options={[
                ...(templates ?? []).map((one) => ({ value: one.id, label: one.title })),
                { value: NONE, label: "None" },
              ]}
              value={template}
              onValueChange={setTemplate}
              disabled={making}
            />
          )}
        />
      )}
      <div className="get-project-end">
        <p className="get-project-folder">
          Goes into <code className="where">{folder ?? parent ?? "—"}</code>
        </p>
        <Button variant="ghost" isDisabled={making} onPress={choose}>
          Choose a folder…
        </Button>
        <Button variant={ready ? "primary" : "secondary"} type="submit" isDisabled={!ready}>
          Create project
        </Button>
      </div>
      {unpicked && (
        <p className="trouble" role="alert">
          {unpicked}
        </p>
      )}
    </Form>
  );
}
