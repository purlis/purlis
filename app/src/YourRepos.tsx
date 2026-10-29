// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { commands, type ListedRepo, type RepoListing, type RepoReach } from "./bindings";
import { ForgeTokenSignIn } from "./ForgeTokenSignIn";
import { Field, SettingRow } from "./settings/components";
import { Button } from "./ui/button/Button";
import { Badge } from "./ui/badge/Badge";
import { CardButton } from "./ui/card/CardButton";

const NOTHING: RepoListing = { repos: [], reaches: [] };

/**
 * Whether `repo` is one the person searched for: every word of `query` is in its owner or its
 * name (`owner/name`, so a word with a slash in it finds the pair). Case does not matter.
 */
export function matches(repo: ListedRepo, query: string): boolean {
  const path = `${repo.owner}/${repo.name}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((word) => path.includes(word));
}

/** The forge a host's sign-in opens on: the dialog's own words for it. */
function forgeOf(reach: RepoReach): "github" | "gitlab" {
  return reach.kind === "gitlab" ? "gitlab" : "github";
}

/**
 * **Your repos, to get a project from** (#1818, spec #1810): beside Get a project's URL box,
 * what every forge account the person signed in to in purlis reaches, purlis projects on top
 * and marked, searchable by name and owner.
 *
 * - **Read as the person, over purlis's own sign-in** (`forge_repo_list`, #1816): nothing here
 *   uses `gh` or `glab`. A project is found by its manifest at the repo's top level, within the
 *   account's request budget, so a repo whose top level was not read is listed unmarked.
 * - **Picking one** hands its address to the page, which clones it and opens it through the
 *   trust question, as a pasted URL is (#1815).
 * - **A host with no account, or one that would not list,** says what to do: Sign in, inline,
 *   with a token (#1814), after which the list is read again; or paste a URL above.
 */
export function YourRepos({
  onPick,
  busy,
}: {
  /** Gets the repo picked: its address, cloned and opened as a pasted one. */
  onPick: (url: string) => void;
  /** A clone is running: nothing else is picked meanwhile. */
  busy: boolean;
}) {
  const [found, setFound] = useState<RepoListing | { refused: string }>();
  const [query, setQuery] = useState("");
  /** The forge whose token sign-in is open here, if one is. */
  const [signingIn, setSigningIn] = useState<"github" | "gitlab">();
  const listLabel = useId();
  /** The newest listing out: an older answer arriving late is dropped. */
  const asking = useRef(0);
  const ask = useCallback(() => {
    const mine = ++asking.current;
    void commands
      .forgeRepoList()
      .then((said) => (said.status === "ok" ? (said.data ?? NOTHING) : { refused: said.error }))
      .catch((err: unknown) => ({ refused: String(err) }))
      .then((answer) => {
        if (asking.current === mine) setFound(answer);
      });
  }, []);
  useEffect(ask, [ask]);
  /** Asks again, the list put away meanwhile: after a sign-in, and Retry. */
  const askAgain = useCallback(() => {
    setFound(undefined);
    ask();
  }, [ask]);

  const listing = found !== undefined && "repos" in found ? found : undefined;
  const shown = listing?.repos.filter((repo) => matches(repo, query)) ?? [];
  const searching = query.trim() !== "";

  return (
    <section className="your-repos" aria-labelledby={listLabel}>
      <h2 id={listLabel}>Or pick one of your repos</h2>

      {signingIn !== undefined && (
        <ForgeTokenSignIn
          forge={signingIn}
          onCancel={() => setSigningIn(undefined)}
          onSignedIn={() => {
            setSigningIn(undefined);
            askAgain();
          }}
        />
      )}

      <p className="pending" role="status" aria-busy={found === undefined}>
        {found === undefined && (
          <>
            <LoaderCircle className="node-icon spinning" aria-hidden />
            Asking your forges which repos you can reach…
          </>
        )}
        {found !== undefined && "refused" in found && found.refused}
      </p>

      {listing !== undefined && listing.repos.length > 0 && (
        <>
          <div className="repo-picker-head">
            <SettingRow
              label="Search your repos"
              setting="your-repos-search"
              help="By name or owner."
              control={(ids) => <Field ids={ids} kind="text" value={query} onChange={setQuery} />}
            />
          </div>
          {shown.length > 0 ? (
            <ul className="repo-picks" aria-label="Your repos">
              {shown.map((repo) => (
                <li key={`${repo.host} ${repo.path}`} data-project={repo.project}>
                  <CardButton
                    variant="pick"
                    title={repo.name}
                    isDisabled={busy}
                    aria-label={`Get ${repo.path}${repo.project ? ", a purlis project," : ""} from ${repo.host} as ${repo.login}`}
                    onPress={() => onPick(repo.url)}
                  >
                    <span>{repo.owner}</span>
                    {repo.project && <Badge>purlis project</Badge>}
                  </CardButton>{" "}
                  <span className="m-0 text-text-secondary">
                    {repo.host} · {repo.login}
                  </span>
                </li>
              ))}
            </ul>
          ) : (
            <p>
              No repo matches “{query.trim()}”.{" "}
              {searching && "Paste its URL above instead, or sign in as another account below."}
            </p>
          )}
        </>
      )}

      {listing !== undefined && (
        <ul className="your-repos-hosts" aria-label="Your forges">
          {listing.reaches.map((reach) => (
            <li key={`${reach.host} ${reach.login ?? ""}`}>
              {reach.trouble ??
                (reach.listed === 0
                  ? `${reach.login} reaches no repos on ${reach.host}. Paste a project's URL above, or sign in as another account.`
                  : `${reach.listed} ${reach.listed === 1 ? "repo" : "repos"} on ${reach.host} as ${reach.login}.`)}{" "}
              {(reach.login === null || reach.trouble !== null) && (
                <Button
                  variant="link"
                  onPress={() => setSigningIn(forgeOf(reach))}
                  className="welcome-link"
                >
                  {reach.login === null ? "Sign in…" : "Sign in again…"}
                </Button>
              )}
              {reach.login !== null && reach.trouble !== null && (
                <Button variant="link" onPress={askAgain} className="welcome-link">
                  Retry
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
