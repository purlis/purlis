// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useState } from "react";
import { commands } from "./bindings";
import { type ForgeAsk, ForgeQuestion } from "./ForgeQuestion";
import { useNewerTrouble } from "./pickTrouble";
import { Choice, Field, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";

/**
 * Making a new project — a plane charter scaffolds — and the one decision it asks about.
 *
 * **The prompt IS the prompt** (ADR 0035). The CLI's shape is `mkdir`, `cd`, `charter
 * init`, because a hook blocked on stdin hangs a turn and `util.py` reads none; here there is a
 * window and a person looking at it, so the directory is picked and the question is asked where
 * the answer is given. Nothing is copied from the printed two-command shape.
 *
 * **The default writes nothing into anybody's repository** (ADR 0035, spec decision 27).
 * `charter init` used to scaffold the plane into whatever directory it was run in, and offer to
 * clone that repo as the first one. The opener destroyed the case that rested on: a directory
 * chosen in a file dialog has nobody standing in it, and writing `purlis.toml`, `personas/`
 * and a block of rules into a tracked `.gitignore` is a write nobody typed. So a directory that
 * is the top of a git repository is refused, in the core's own four-line sentence, which names
 * the plane-beside-it shape and the flag that asks for the old one.
 *
 * **"Make this repo itself the project" is that flag, as a box.** charter's own plane is a
 * repository, which is why the option is here at all — and it is never the default, and never
 * ticked for the operator.
 *
 * **Adopting a repository is the other half, and it is the one ADR 0035 calls the default**
 * (charter-app#175). The record's sentence is that `charter init` on an existing repo *"adopts
 * that repo as the plane's first clone and makes the plane beside it"*, so this asks for two
 * directories rather than showing a refusal about them: where the plane goes, and which repo
 * it starts with. The repo is cloned into `workspaces/<default>/<name>/` with charter's git
 * policy applied to the clone, and **nothing is written into the repo itself** — it is read,
 * and only read.
 *
 * The two answers are separate boxes because they are separate answers. Deriving the plane's
 * directory from the repo's (`../<name>-plane`) would put charter's guess where the operator's
 * decision belongs, on the one field this dialog exists to collect.
 *
 * Whatever is scaffolded is then opened **through the trust gate** — see `create_project`. A
 * plane charter has just made is still a plane this machine has approved nothing about, so the
 * ordinary end of this dialog is the approval dialog, on the same path a recents row takes.
 *
 * **All of the above is under Advanced now** (FR-4, #603, ruling W10). The default asks one
 * thing, the repository, and opens it into this machine's local project in a workspace named
 * after it — the first run's own path, so nobody is asked where a plane goes before they have a
 * reason to care. The two-directory form is a `<details>`, closed, because the browser has a
 * collapsible (`docs/ui-primitives.md`).
 *
 * **Every answer is drawn from the settings set** (DS-3c, #1175; ADR 0037's 2026-10-04
 * amendment): a {@link SettingRow} holding a {@link Field} or the box as a {@link Choice}, so a
 * line of help is its box's own description. A `Browse…` sits in its box's row, beside it.
 */
export function NewProject({
  /** Why the last attempt made nothing — **the core's lines, unchanged and all of them**. */
  trouble,
  /** Which forge the project's repos are on, when the repo's remote did not say (#839). */
  forgeAsk,
  /** The form changed, so a pending question about the forge is about a form that is gone. */
  onEdit,
  /** Whether charter is making it right now, so the answer cannot be given twice. */
  making,
  onCreate,
  onOpenRepo,
  opening,
  onCancel,
}: {
  trouble?: string;
  forgeAsk?: ForgeAsk;
  onEdit?: () => void;
  making: boolean;
  onCreate: (path: string, planeIsThisRepo: boolean, adopt: string) => void;
  /** Opens a repo into this machine's local project (FR-4). */
  onOpenRepo: (path: string) => void;
  /** Whether that is happening right now. */
  opening: boolean;
  onCancel: () => void;
}) {
  // A question about the forge is about the answers that were sent. Once one of them changes,
  // the next press asks again rather than answering for a form nobody sent (#848 review).
  const [repo, keepRepo] = useState("");
  const [path, keepPath] = useState("");
  const [adopt, keepAdopt] = useState("");
  const [planeIsThisRepo, keepPlaneIsThisRepo] = useState(false);
  const setRepo = (value: string) => {
    keepRepo(value);
    onEdit?.();
  };
  const setPath = (value: string) => {
    keepPath(value);
    onEdit?.();
  };
  const setAdopt = (value: string) => {
    keepAdopt(value);
    onEdit?.();
  };
  const setPlaneIsThisRepo = (value: boolean) => {
    keepPlaneIsThisRepo(value);
    onEdit?.();
  };
  const ready = path.trim() !== "" && !making;

  // One picker, told where to put its answer. Both fields ask the same question of the same
  // file dialog — which directory — and two copies of it would be two places to fix the day
  // a cancelled pick stops answering null.
  //
  // A cancelled dialog is null and is not a failure: nothing is said and nothing moves. A dialog
  // that could not open is said where the core's refusal is (#1291), until the next try or send,
  // or until a newer refusal comes (a forge answer sends the form again on its own).
  const { said, pickFailed, started } = useNewerTrouble(trouble);
  const pick = useCallback(
    (into: (chosen: string) => void) => {
      started();
      void commands
        .pickProject()
        .catch((err: unknown) => ({ status: "error" as const, error: String(err) }))
        .then((answer) => {
          if (answer.status === "error") pickFailed(answer.error);
          else if (answer.data) into(answer.data);
        });
    },
    [pickFailed, started],
  );

  // **One main action, the one for the part the person is in**: Open repo, or, with Advanced
  // open, Create project. It stays in its place and keeps the room of the longer, so opening
  // Advanced moves nothing.
  const [advanced, setAdvanced] = useState(false);
  const openRepo = () => {
    started();
    if (repo.trim() !== "" && !opening) onOpenRepo(repo.trim());
  };
  const create = () => {
    started();
    // The box and the adopt field are two answers to one question — which repository this plane
    // starts from — so a ticked box sends no repo, rather than sending both and letting the core
    // rank them.
    if (ready) onCreate(path.trim(), planeIsThisRepo, planeIsThisRepo ? "" : adopt.trim());
  };
  return (
    <Dialog
      onClose={onCancel}
      title="New project"
      titleId="new-project"
      main={
        // While a project of its own is made, its act says so, whatever is open.
        advanced || making
          ? {
              children: making ? "Creating…" : "Create project",
              reserve: ["Open repo", "Create project", "Creating…"],
              isDisabled: !ready,
              onPress: create,
            }
          : {
              children: "Open repo",
              reserve: ["Open repo", "Create project", "Creating…"],
              isDisabled: repo.trim() === "" || opening,
              onPress: openRepo,
            }
      }
    >
      <p className="came-back">
        Pick a repo. purlis opens it in a workspace of its own, in the project it keeps on this
        machine. Nothing is written into your repo.
      </p>

      <SettingRow
        label="Repo"
        control={(ids) => (
          <>
            <Field
              kind="text"
              ids={ids}
              value={repo}
              placeholder="/where/the/repo/is"
              onChange={setRepo}
            />
            <Button aria-label="Browse for the repo" onPress={() => pick(setRepo)}>
              Browse…
            </Button>
          </>
        )}
      />
      {/* The core's refusal, all of it, for whichever form was sent last. */}
      {said && (
        <p className="trouble said-in-full" role="alert">
          {said}
        </p>
      )}
      {/* What it is doing, in the first run's words, while a repo opens (#630). */}
      {opening && (
        <p className="pending" role="status" aria-busy="true">
          Copying your repo into its workspace…
        </p>
      )}
      {/* Asked for whichever form was sent last, as the refusal is (#839). */}
      {forgeAsk && <ForgeQuestion ask={forgeAsk} />}

      {/* The two-directory form (ADR 0035), for a project of its own somewhere the operator
          chooses. Closed until asked for. */}
      <details
        className="advanced"
        onToggle={(event) => setAdvanced((event.currentTarget as HTMLDetailsElement).open)}
      >
        <summary tabIndex={0}>Advanced</summary>
        <p className="came-back">
          A project of its own, in a folder you choose, holding workspaces, personas and the repos
          work happens in.
        </p>
        <SettingRow
          label="Folder"
          help="It does not have to exist yet. purlis makes it, and writes the project into it."
          control={(ids) => (
            <>
              <Field
                kind="text"
                ids={ids}
                value={path}
                placeholder="/where/it/goes"
                onChange={setPath}
              />
              {/* **The name says which box it fills**, because there are two of them. Two
                  buttons reading `Browse…` announce identically and pick different directories,
                  which is a question a screen reader cannot answer and a sighted operator answers
                  only from where the button sits. */}
              <Button aria-label="Browse for the folder" onPress={() => pick(setPath)}>
                Browse…
              </Button>
            </>
          )}
        />

        {/* ADR 0035's default, as a second directory rather than a refusal about one. */}
        <SettingRow
          label="Repo to adopt"
          help={
            <>
              Optional: the project goes in the folder above, and this repo is cloned into its{" "}
              <code>workspaces/</code>. Nothing is written into the repo — it is read, and only
              read. Leave it empty for a project with no repos yet.
            </>
          }
          control={(ids) => (
            <>
              <Field
                kind="text"
                ids={ids}
                value={adopt}
                placeholder="/where/the/repo/is"
                disabled={planeIsThisRepo}
                onChange={setAdopt}
              />
              <Button
                aria-label="Browse for the repo to adopt"
                isDisabled={planeIsThisRepo}
                onPress={() => pick(setAdopt)}
              >
                Browse…
              </Button>
            </>
          )}
        />

        {/* The one decision, and it is the operator's: the set's toggle, because it is the one
            answer on this dialog that writes into a repository the operator already has. */}
        <SettingRow
          label="Make this repo itself the project"
          help={
            <>
              Only for a folder that is the top of a git repo, and only when you mean it: it writes{" "}
              <code>purlis.toml</code>, <code>personas/</code>, <code>workspaces/</code> and
              purlis&rsquo;s rules into that repo&rsquo;s tracked <code>.gitignore</code>.
              purlis&rsquo;s own project is one of these. Left unticked, purlis writes nothing into
              a repo and says how to make a project beside it.
            </>
          }
          control={(ids) => (
            <Choice
              kind="toggle"
              ids={ids}
              checked={planeIsThisRepo}
              onCheckedChange={setPlaneIsThisRepo}
            />
          )}
        />

        {/* `init`'s refusal in a repository is four lines — what it will not do, the commands
            that make a plane beside the repo, what asking for the old shape would write, and
            where the decision is recorded — and it is drawn in full above, where every refusal
            this dialog is given goes. */}
      </details>
    </Dialog>
  );
}
