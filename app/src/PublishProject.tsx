// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { Channel } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type CreateProgress, type PlaneId } from "./bindings";
import { FORGE_NAME, OwnerAndVisibility, StagesShown, useForgeOwners } from "./CreateOnForge";
import { askSettingsLink } from "./settings/links";
import { Choice, Field, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";

/** The forges a project is published to. */
type Forge = keyof typeof FORGE_NAME;

/** The window event a publish that landed sends, with the project's id: every line that says
 *  the project lives only here asks again, and goes. */
export const PUBLISHED = "purlis-project-published";

/** What the dialog is doing. */
type Doing =
  | { kind: "asking" }
  | { kind: "publishing"; progress?: CreateProgress }
  | { kind: "refused"; sentence: string }
  | { kind: "called-off" }
  | { kind: "published"; at: string };

/**
 * The page on the forge where a repo purlis could not delete is deleted by hand, as purlis's
 * own sentence names it (`createproject::repo_left`): its settings on GitHub, its edit page on
 * GitLab. `undefined` when the sentence names none.
 */
export function settingsPageIn(sentence: string): string | undefined {
  return /https:\/\/[^\s,]+\/(?:settings|edit)(?=[\s,.]|$)/.exec(sentence)?.[0];
}

/**
 * **Publish a project that lives only on this machine to GitHub or GitLab** (#1820, ruling Q9):
 * the dialog the "lives only on this machine" line opens, on the project's tab and in its
 * settings.
 *
 * It asks what Create on a forge asks (#1819's form): which forge, a name (the folder's, made
 * usable), the owner from the person's purlis sign-in, and who can see it, private unless they
 * pick public. Publish runs the core's stages, each listed as it runs with Cancel beside them:
 * what is not committed is committed, the repo is made, the project's history is pushed through
 * purlis's own credential helper and recorded as the project's remote. Cancel, a failure, or
 * closing the dialog mid-way takes back what the publish made; a failure says why, with Retry,
 * and where the forge kept a repo it would not delete, a link to that repo's settings, and the
 * Retry pushes into it.
 *
 * With no sign-in to the forge picked, it says so and offers Settings › You › Accounts.
 */
export function PublishProject({
  plane,
  suggested,
  onClose,
}: {
  plane: PlaneId;
  /** The repo's name the core suggests: the project folder's, made usable. */
  suggested: string;
  onClose: () => void;
}) {
  const [forge, setForge] = useState<Forge>("github");
  const forgeName = FORGE_NAME[forge];
  const { owners, owner, setOwner, retry } = useForgeOwners(forge);
  const [name, setName] = useState(suggested);
  const [visibility, setVisibility] = useState("private");
  const [doing, setDoing] = useState<Doing>({ kind: "asking" });
  const publishingNow = useRef(false);
  const publishing = doing.kind === "publishing";
  const typed = name.trim();
  const listed = owners.kind === "listed" ? owners.owners : undefined;
  const ready = !!listed && !!owner && typed !== "" && !publishing;

  // Closing the dialog while it publishes calls the publish off, so nothing is left half-made.
  useEffect(
    () => () => {
      if (publishingNow.current) void commands.cancelCreateProject();
    },
    [],
  );

  const publish = useCallback(async () => {
    if (!owner || typed === "") return;
    const progress = new Channel<CreateProgress>();
    progress.onmessage = (now) =>
      setDoing((was) => (was.kind === "publishing" ? { kind: "publishing", progress: now } : was));
    publishingNow.current = true;
    setDoing({ kind: "publishing" });
    const answer = await commands
      .publishProject(
        plane,
        { forge, owner, name: typed, private: visibility === "private" },
        progress,
      )
      .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
    publishingNow.current = false;
    if (answer.status === "error") setDoing({ kind: "refused", sentence: answer.error });
    else if (answer.data === null) setDoing({ kind: "called-off" });
    else {
      setDoing({ kind: "published", at: answer.data });
      window.dispatchEvent(new CustomEvent<string>(PUBLISHED, { detail: plane }));
    }
  }, [plane, forge, owner, typed, visibility]);

  const settingsPage = doing.kind === "refused" ? settingsPageIn(doing.sentence) : undefined;

  const published = doing.kind === "published";
  return (
    <Dialog
      onClose={onClose}
      title="Publish to GitHub or GitLab"
      titleId="publish-project"
      cancel={
        published
          ? { children: "Done" }
          : // While it publishes, its stages carry their own Cancel, which takes everything back.
            { children: "Close", isDisabled: publishing }
      }
      main={
        published
          ? undefined
          : { children: "Publish", isDisabled: !ready, onPress: () => void publish() }
      }
    >
      {published ? (
        <p className="came-back" role="status">
          Published to <code className="where">{doing.at}</code>. It is this project's remote now:
          saves push there.
        </p>
      ) : (
        <>
          <p className="came-back">
            purlis makes a new repo on {forgeName} with your own account, pushes this project&apos;s
            history to it, and makes it the project&apos;s remote. Cancel takes back everything it
            made.
          </p>
          <div aria-busy={publishing}>
            <SettingRow
              label="Where it goes"
              setting="publish-project-forge"
              grouped
              control={(ids) => (
                <Choice
                  ids={ids}
                  kind="radio"
                  options={[
                    { value: "github", label: "GitHub", says: "A new repo on GitHub." },
                    { value: "gitlab", label: "GitLab", says: "A new repo on GitLab." },
                  ]}
                  value={forge}
                  onValueChange={(to) => {
                    setForge(to as Forge);
                    setOwner(undefined);
                    if (!publishing) setDoing({ kind: "asking" });
                  }}
                  disabled={publishing}
                />
              )}
            />
            {owners.kind === "signed-out" ? (
              <p className="came-back">
                purlis publishes with your own {forgeName} account, and this machine is not signed
                in to {forgeName}.{" "}
                <Button
                  onPress={() => {
                    askSettingsLink(plane, { group: "you.accounts" });
                    onClose();
                  }}
                >
                  Sign in to {forgeName}…
                </Button>
              </p>
            ) : (
              <>
                <SettingRow
                  label="Name"
                  setting="publish-project-name"
                  help="The repo's name: letters, digits, dots, dashes and underscores."
                  control={(ids) => (
                    <Field
                      ids={ids}
                      kind="text"
                      value={name}
                      placeholder="my-project"
                      onChange={(to) => {
                        setName(to);
                        if (!publishing) setDoing({ kind: "asking" });
                      }}
                    />
                  )}
                />
                <OwnerAndVisibility
                  forge={forge}
                  owners={owners}
                  owner={owner}
                  onOwner={setOwner}
                  visibility={visibility}
                  onVisibility={setVisibility}
                  busy={publishing}
                  onRetry={retry}
                />
              </>
            )}
          </div>

          <div role="status" className="create-on-forge-progress">
            {doing.kind === "publishing" && (
              <StagesShown
                label={`Publishing ${typed}`}
                progress={doing.progress}
                onCancel={() => void commands.cancelCreateProject()}
              />
            )}
            {doing.kind === "called-off" && "Cancelled. Nothing was left behind."}
          </div>

          {doing.kind === "refused" && (
            <p className="trouble said-in-full" role="alert">
              {doing.sentence}{" "}
              {settingsPage && (
                <Button onPress={() => void openUrl(settingsPage).catch(() => undefined)}>
                  Open the repo&apos;s settings
                </Button>
              )}{" "}
              <Button isDisabled={!ready} onPress={() => void publish()}>
                Retry
              </Button>
            </p>
          )}
        </>
      )}
    </Dialog>
  );
}
