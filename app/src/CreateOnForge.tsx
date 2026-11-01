// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import clsx from "clsx";
import { Channel } from "@tauri-apps/api/core";
import { commands, type CreateProgress, type ForgeOwners, type TemplateRow } from "./bindings";
import { REMEMBERED_PARENT } from "./GetProject";
import { Choice, Field, SettingActions, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Form } from "./ui/field/Field";

/** Where a project lives: on a forge, or only on this machine. */
export type Where = "mac" | "github" | "gitlab";

/** What each forge is called. */
export const FORGE_NAME: Record<Exclude<Where, "mac">, string> = {
  github: "GitHub",
  gitlab: "GitLab",
};

/** The radio value that is no template: template ids are lower-case words, so it never clashes. */
const EMPTY = ":empty";

/** The place picked last to hold projects: the one Get a project remembers too (ruling Q7). */
export function rememberedParent(): string | undefined {
  try {
    return globalThis.localStorage?.getItem(REMEMBERED_PARENT) ?? undefined;
  } catch {
    return undefined;
  }
}

export function rememberParent(parent: string): void {
  try {
    globalThis.localStorage?.setItem(REMEMBERED_PARENT, parent);
  } catch {
    // A window that cannot keep it asks again next time; the project itself is unaffected.
  }
}

/** Who the picker can offer: still asking, no sign-in, the forge's answer, or why not. */
export type Owners =
  | { kind: "asking" }
  | { kind: "signed-out" }
  | { kind: "listed"; owners: ForgeOwners }
  | { kind: "unread"; sentence: string };

/**
 * **Whom the person's account on `forge` can make a repo for**, asked of the forge each time a
 * form shows it, and again on `retry`: the owner picked starts as the account itself. Shared by
 * Create (#1819) and Publish (#1820).
 */
export function useForgeOwners(forge: Exclude<Where, "mac">) {
  const [owners, setOwners] = useState<Owners>({ kind: "asking" });
  const [asked, setAsked] = useState(0);
  const [owner, setOwner] = useState<string>();
  useEffect(() => {
    let gone = false;
    void commands
      .forgeOwners(forge)
      .then((answer) => {
        if (gone) return;
        if (answer.status === "error") setOwners({ kind: "unread", sentence: answer.error });
        else if (answer.data.host === null) setOwners({ kind: "signed-out" });
        else {
          setOwners({ kind: "listed", owners: answer.data });
          setOwner((was) => was ?? answer.data.login);
        }
      })
      .catch((err: unknown) => {
        if (!gone) setOwners({ kind: "unread", sentence: String(err) });
      });
    return () => {
      gone = true;
    };
  }, [forge, asked]);
  const retry = useCallback(() => setAsked((n) => n + 1), []);
  return { owners, owner, setOwner, retry };
}

/**
 * **The owner and who can see it**, #1819's form rows that Publish (#1820) asks too: the owner
 * from the forge's answer (or why it could not be read, with Retry), and private or public,
 * private unless the person picks public.
 */
export function OwnerAndVisibility({
  forge,
  owners,
  owner,
  onOwner,
  visibility,
  onVisibility,
  busy,
  onRetry,
}: {
  forge: Exclude<Where, "mac">;
  owners: Owners;
  owner: string | undefined;
  onOwner: (to: string) => void;
  visibility: string;
  onVisibility: (to: string) => void;
  busy: boolean;
  onRetry: () => void;
}) {
  const name = FORGE_NAME[forge];
  const listed = owners.kind === "listed" ? owners.owners : undefined;
  return (
    <>
      {owners.kind === "asking" && (
        <p className="pending" role="status" aria-busy="true">
          Asking {name} whom you can create for…
        </p>
      )}
      {owners.kind === "unread" && (
        <p className="trouble" role="alert">
          {owners.sentence} <Button onPress={onRetry}>Retry</Button>
        </p>
      )}
      {listed && (
        <SettingRow
          label="Owner"
          setting="create-project-owner"
          help={`You, or an organisation you belong to on ${name}.`}
          control={(ids) => (
            <Choice
              ids={ids}
              kind="select"
              options={listed.owners.map((one) => ({
                value: one.name,
                label: one.mine ? `${one.title} (you)` : one.title,
              }))}
              value={owner}
              onValueChange={onOwner}
              disabled={busy}
            />
          )}
        />
      )}
      <SettingRow
        label="Who can see it"
        grouped
        control={(ids) => (
          <Choice
            ids={ids}
            kind="radio"
            options={[
              { value: "private", label: "Private", says: "Only you and those you invite." },
              { value: "public", label: "Public", says: `Everyone on ${name} can read it.` },
            ]}
            value={visibility}
            onValueChange={onVisibility}
            disabled={busy}
          />
        )}
      />
    </>
  );
}

/**
 * **Every stage of a run, as it runs**, with Cancel beside them: the one running marked as the
 * current step with an ellipsis, those before it done. Shared by Create and Publish.
 */
export function StagesShown({
  label,
  progress,
  onCancel,
}: {
  label: string;
  progress: CreateProgress | undefined;
  onCancel: () => void;
}) {
  return (
    <>
      <ol aria-label={label}>
        {(progress?.stages ?? []).map((stage, at) => (
          <li
            key={stage}
            aria-current={at === progress?.at ? "step" : undefined}
            className={clsx(at < (progress?.at ?? 0) && "done")}
          >
            {stage}
            {at === progress?.at ? "…" : ""}
          </li>
        ))}
      </ol>
      <Button onPress={onCancel}>Cancel</Button>
    </>
  );
}

/** What the page is doing. */
type Doing =
  | { kind: "asking" }
  | { kind: "creating"; progress?: CreateProgress }
  | { kind: "refused"; sentence: string }
  | { kind: "called-off" };

/**
 * **Create a project on GitHub or GitLab, in one step** (#1819, ruling Q8): the Create a
 * project page when the person picks a forge for it to live on.
 *
 * - **The form**: a name; an owner, from the account and its organisations or groups, as the
 *   forge lists them through the person's purlis sign-in; private unless the person picks
 *   public; a stack template (FR-17) or an empty project. It goes into `~/Projects/<name>`, or
 *   the place picked last with Choose a folder…, the same place Get a project remembers.
 * - **No sign-in to that forge**: the page says so and offers Sign in, which the guided steps
 *   turn into their sign-in step (`onNeedsSignIn`); after it, the person is back here.
 * - **Create**: the repo is made, the template laid out, the first commit pushed through
 *   purlis's own credential helper, and the project opened through the trust question, with
 *   each stage listed as it runs and Cancel beside them. Cancel, a failure, or leaving the page
 *   takes back what the creation made: the repo on the forge and the folder. A failure says why
 *   and what was taken back, with Retry.
 */
export function CreateOnForge({
  forge,
  templates,
  onOpen,
  onNeedsSignIn,
  withGit = (act) => void act(),
}: {
  forge: Exclude<Where, "mac">;
  /** The templates this purlis ships, once the machine has been read. */
  templates: readonly TemplateRow[] | undefined;
  /** Opens a folder through the trust question: the welcome's own open. */
  onOpen: (path: string) => void;
  /** No sign-in to this forge: the sign-in, then back here. */
  onNeedsSignIn: () => void;
  /** Runs the creation once git is there (#1813's gate, held by the guided steps). */
  withGit?: (act: () => unknown) => void;
}) {
  const name = FORGE_NAME[forge];
  const { owners, owner, setOwner, retry } = useForgeOwners(forge);
  const [project, setProject] = useState("");
  const [visibility, setVisibility] = useState("private");
  const [template, setTemplate] = useState(EMPTY);
  const [parent, setParent] = useState<string | undefined>(rememberedParent);
  /** Where the typed name's project goes, by the name it was asked for, or why it cannot. */
  const [answered, setAnswered] = useState<{ name: string; folder?: string; why?: string }>();
  const [doing, setDoing] = useState<Doing>({ kind: "asking" });
  const [unpicked, setUnpicked] = useState<string>();
  const creatingNow = useRef(false);
  const creating = doing.kind === "creating";
  const typed = project.trim();
  const folder = answered?.name === typed ? answered : undefined;

  // The folder follows the name as it is typed; a name that cannot be one says why.
  useEffect(() => {
    let gone = false;
    if (!typed) return;
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
      .catch(() => undefined);
    return () => {
      gone = true;
    };
  }, [typed, parent]);

  // Leaving the page while it creates calls the creation off, so nothing is left half-made.
  useEffect(
    () => () => {
      if (creatingNow.current) void commands.cancelCreateProject();
    },
    [],
  );

  const create = useCallback(async () => {
    if (!owner || !folder?.folder) return;
    const progress = new Channel<CreateProgress>();
    progress.onmessage = (now) =>
      setDoing((was) => (was.kind === "creating" ? { kind: "creating", progress: now } : was));
    creatingNow.current = true;
    setDoing({ kind: "creating" });
    const answer = await commands
      .createProjectOnForge(
        {
          forge,
          owner,
          name: typed,
          private: visibility === "private",
          template: template === EMPTY ? null : template,
          folder: folder.folder,
        },
        progress,
      )
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    creatingNow.current = false;
    if (answer.status === "error") setDoing({ kind: "refused", sentence: answer.error });
    else if (answer.data === null) setDoing({ kind: "called-off" });
    else {
      setDoing({ kind: "asking" });
      onOpen(answer.data);
    }
  }, [forge, owner, folder, typed, visibility, template, onOpen]);

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

  if (owners.kind === "signed-out")
    return (
      <div className="create-on-forge">
        <p className="welcome-lede">
          purlis creates the repo on {name} with your own {name} account. Sign in to {name} first;
          you come back here after.
        </p>
        <SettingActions>
          <Button onPress={onNeedsSignIn}>Sign in to {name}</Button>
        </SettingActions>
      </div>
    );

  const listed = owners.kind === "listed" ? owners.owners : undefined;
  const ready = !!listed && !!owner && !!folder?.folder && !creating;

  return (
    <div className="create-on-forge">
      <p className="welcome-lede">
        purlis creates the repo on {name}
        {listed?.host ? ` (${listed.host}, as ${listed.login})` : ""}, lays out the project in it,
        pushes the first commit and opens it. Cancel takes back everything it made.
      </p>
      <Form
        aria-busy={creating}
        onSubmit={(event) => {
          event.preventDefault();
          if (ready) withGit(create);
        }}
      >
        <SettingRow
          label="Name"
          setting="create-project-name"
          help="Letters, digits, dots, dashes and underscores, like my-project."
          control={(ids) => (
            <Field
              ids={ids}
              kind="text"
              value={project}
              placeholder="my-project"
              onChange={(to) => {
                setProject(to);
                if (!creating) setDoing({ kind: "asking" });
              }}
            />
          )}
        />
        {folder?.why && (
          <p className="trouble" role="alert">
            {folder.why}
          </p>
        )}
        <OwnerAndVisibility
          forge={forge}
          owners={owners}
          owner={owner}
          onOwner={setOwner}
          visibility={visibility}
          onVisibility={setVisibility}
          busy={creating}
          onRetry={retry}
        />
        {templates && templates.length > 0 && (
          <SettingRow
            label="Project template"
            help="Personas, a review checklist and the commands purlis asks you about, for the stack you work in."
            control={(ids) => (
              <Choice
                ids={ids}
                kind="select"
                options={[
                  { value: EMPTY, label: "Empty", says: "Only the steward persona." },
                  ...templates.map((one) => ({
                    value: one.id,
                    label: one.title,
                    says: one.summary,
                  })),
                ]}
                value={template}
                onValueChange={setTemplate}
                disabled={creating}
              />
            )}
          />
        )}
        <div className="get-project-end">
          <p className="get-project-folder">
            Goes into <code className="where">{folder?.folder ?? "—"}</code>
          </p>
          <Button variant="ghost" isDisabled={creating} onPress={choose}>
            Choose a folder…
          </Button>
          <Button variant={ready ? "primary" : "secondary"} type="submit" isDisabled={!ready}>
            Create project
          </Button>
        </div>
      </Form>

      <div role="status" className="create-on-forge-progress">
        {doing.kind === "creating" && (
          <>
            <StagesShown
              label={`Creating ${typed}`}
              progress={doing.progress}
              onCancel={() => void commands.cancelCreateProject()}
            />
          </>
        )}
        {doing.kind === "called-off" && "Cancelled. Nothing was left behind."}
      </div>

      {doing.kind === "refused" && (
        <p className="trouble said-in-full" role="alert">
          {doing.sentence} <Button onPress={() => withGit(create)}>Retry</Button>
        </p>
      )}
      {unpicked && (
        <p className="trouble" role="alert">
          {unpicked}
        </p>
      )}
    </div>
  );
}

/**
 * **Where the project lives** (ruling Q2): GitHub, GitLab, or only this machine. "Only this
 * Mac" is the project purlis keeps here, with no account at all, and the page says plainly that
 * it is on this machine only (ruling Q9).
 */
export function WhereItLives({ value, onChange }: { value: Where; onChange: (to: Where) => void }) {
  return (
    <SettingRow
      label="Where it lives"
      setting="create-project-where"
      grouped
      control={(ids) => (
        <Choice
          ids={ids}
          kind="cards"
          options={[
            {
              value: "mac",
              label: "Only this Mac",
              says: "No account needed, and no copy anywhere else.",
            },
            {
              value: "github",
              label: "GitHub",
              says: "A new repo on your account.",
            },
            {
              value: "gitlab",
              label: "GitLab",
              says: "A new project on your account.",
            },
          ]}
          value={value}
          onValueChange={(to) => onChange(to as Where)}
        />
      )}
    />
  );
}
