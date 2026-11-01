// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from "react";
import { useEffect, useState } from "react";
import { commands, type ForgeAccountRow, type ForgeRow } from "./bindings";
import { DoorPage } from "./DoorPage";
import { ForgeBrowserSignIn } from "./ForgeBrowserSignIn";
import { forgeAccountsChanged } from "./ForgeSignInEnded";
import { ForgeTokenSignIn } from "./ForgeTokenSignIn";
import { ImportCliLogin } from "./ImportCliLogin";
import { Button } from "./ui/button/Button";

/**
 * **Sign in to GitHub or GitLab** (#1812's entry; the sign-in itself is #1814's, then the
 * browser sign-ins after the app registration, #726).
 *
 * Signing in is skippable and is asked again, inline, when an act needs it (ruling Q2), so the
 * welcome draws it as a quieter entry under its doors rather than as a fourth door. **Sign in
 * with GitHub** is the browser sign-in on github.com ({@link ForgeBrowserSignIn}, #729): a code
 * shown here and confirmed on GitHub's own page. Beside it, **Sign in with a token** opens
 * purlis's own token sign-in ({@link ForgeTokenSignIn}, #1814): any host, and the only way on a
 * forge a team hosts. Either way the sign-in is kept in the keychain and never reaches a chat.
 * Below, what also works: each forge's own command-line tool, `gh` or `glab`, signed in on this machine,
 * which a host with no purlis sign-in falls back to and which chats always use (#1816). A tool
 * that is installed and not signed in has its sign-in one press away, typed into a shell tab in the
 * project purlis keeps here (FR-4's "detected and offered"); one that is not installed says it
 * is only needed for that forge, because purlis never needs either for a project on this
 * machine.
 *
 * The `gh`/`glab` rows stay as the fallback (#1816) until they are only an import. Where one of
 * them is signed in and purlis has no account on its host, the page offers to use that login
 * ({@link ImportCliLogin}, #1376): what the import costs is said before it is pressed, and
 * nothing is imported unless it is.
 */
export function ForgeSignIn({
  onBack,
  onSignInToForge,
  trouble,
  onSignedIn,
  end,
}: {
  /** Comes back to the welcome. */
  onBack: () => void;
  /** Opens the project purlis keeps here with `<cli> auth login` in a shell tab. */
  onSignInToForge: (row: ForgeRow) => void;
  /** Why the last sign-in opened nothing, in the core's words. */
  trouble?: string;
  /** A token signed in here: the guided steps' way on is then Continue (#1817). */
  onSignedIn?: () => void;
  /** The guided steps' way on, at the page's end: Skip for now, or Continue (#1817). */
  end?: ReactNode;
}) {
  const [forges, setForges] = useState<ForgeRow[]>();
  // The window keeps one refusal for every way a repo or the local project is opened. One that
  // was there before this page opened is Create a project's, not a sign-in's, so it is not said
  // here; the window clears it as a sign-in starts, and that sign-in's own refusal is.
  const [before] = useState(trouble);
  const said = trouble !== before ? trouble : undefined;
  /** Why purlis could not look at this machine, in the core's words: said, not swallowed. */
  const [unread, setUnread] = useState<string>();
  /** Whether the token sign-in is open. */
  const [withToken, setWithToken] = useState(false);
  /** The account a sign-in here made, said once it is done. */
  const [signedIn, setSignedIn] = useState<ForgeAccountRow>();

  useEffect(() => {
    let gone = false;
    void commands
      .firstRunFound()
      .then((answer) => {
        if (gone) return;
        if (answer.status === "ok") setForges(answer.data.forges);
        else setUnread(answer.error);
      })
      .catch((err: unknown) => {
        if (!gone) setUnread(String(err));
      });
    return () => {
      gone = true;
    };
  }, []);

  return (
    <DoorPage title="Sign in to GitHub or GitLab" className="forge-sign-in" onBack={onBack}>
      <p className="welcome-lede">
        Sign in to github.com or gitlab.com through your browser, or with a token on either or on a
        forge your team hosts. purlis keeps the sign-in in your system keychain, uses it for the
        forge work you do in this window, and never lets it reach a chat.
      </p>
      <ForgeBrowserSignIn
        onSignedIn={(account) => {
          setSignedIn(account);
          forgeAccountsChanged();
          onSignedIn?.();
        }}
      />
      <p>
        <Button onPress={() => setWithToken(true)}>Sign in with a token…</Button>
      </p>
      {signedIn && (
        <p className="came-back" role="status">
          Signed in to {signedIn.host} as {signedIn.login}
          {signedIn.method === "imported"
            ? `, with your ${signedIn.kind === "gitlab" ? "glab" : "gh"} login`
            : signedIn.method === "browser"
              ? ", through your browser"
              : ""}
          .
        </p>
      )}
      {/* An existing gh or glab login, offered and never taken (#1376, ADR 0077 §5). */}
      <ImportCliLogin
        onImported={(account) => {
          setSignedIn(account);
          onSignedIn?.();
        }}
      />
      {withToken && (
        <ForgeTokenSignIn
          onCancel={() => setWithToken(false)}
          onSignedIn={(account) => {
            setWithToken(false);
            setSignedIn(account);
            onSignedIn?.();
          }}
        />
      )}

      <p>
        Without a purlis sign-in, purlis works with GitHub and GitLab through their own command-line
        tools, <code>gh</code> and <code>glab</code>, signed in on this machine. Chats always use
        them.
      </p>

      {forges === undefined && unread === undefined && (
        <p className="pending" role="status" aria-busy="true">
          Reading what this machine has…
        </p>
      )}
      {unread !== undefined && (
        <p className="came-back" role="status">
          purlis could not read what this machine has: {unread}
        </p>
      )}
      {forges && (
        <>
          <h2 id="forge-sign-in-found">On this machine</h2>
          <ul className="first-run-found" aria-labelledby="forge-sign-in-found">
            {forges.map((row) => (
              <li key={row.cli}>
                <span className="tab-name">{row.cli}</span>: {forgeSays(row)}
                {row.installed && !row.signed_in && (
                  <>
                    {" "}
                    <Button onPress={() => onSignInToForge(row)}>Sign in to {row.title}</Button>
                  </>
                )}
              </li>
            ))}
          </ul>
        </>
      )}

      {/* Verbatim: the core's sentence names what stopped the project from opening. */}
      {said && (
        <p className="trouble said-in-full" role="alert">
          {said}
        </p>
      )}

      {end}
    </DoorPage>
  );
}

/** What a forge's command-line tool is on this machine, and what it is for. */
function forgeSays(row: ForgeRow): string {
  const why = `only needed to work with ${row.title}`;
  if (!row.installed) return `not installed; ${why}`;
  return row.signed_in ? "signed in" : `installed, not signed in; ${why}`;
}
