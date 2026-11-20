// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { commands, type ForgeApplicationRow, type ProjectAccountRow } from "../bindings";
import { ForgeDeviceSignIn } from "../ForgeDeviceSignIn";
import { ForgeGitLabSignIn } from "../ForgeGitLabSignIn";
import { forgeAccountsChanged } from "../ForgeSignInEnded";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { Dialog } from "../ui/dialog/Dialog";
import { Select } from "../ui/select/Select";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "../ui/table/Table";
import { Choice, Field, SettingRow } from "./components";
import type { LiveSetting } from "./groups";

/**
 * **One login, in Settings › You › Accounts** (ADR 0128): the two rows that make "sign in once per
 * forge host" the person's own.
 *
 * - **Your organization's application** ({@link applications}): on a GitHub Enterprise Server host
 *   or a self-managed GitLab, an owner of an organization or group makes one application for purlis
 *   and the person pins it here, with the owner they expect (*Use this application*). Before the
 *   person signs in, the row says what to check on the forge's own page: no API tells purlis who
 *   owns an application, so the person compares. A request to send an owner who has not made one is
 *   one press away.
 * - **Account for each project** ({@link projectAccounts}): with a personal and an
 *   enterprise-managed account on one host, which serves each repo is the person's choice. One
 *   account on a host asks nothing. An account that signed out is never replaced by the other one:
 *   the row says so and the person signs in again or chooses.
 *
 * Both are the window's alone (ADR 0128 D5): the commands are never served on a link.
 */

const FORGES = [
  { value: "github", label: "GitHub", says: "a GitHub Enterprise Server organization's app" },
  { value: "gitlab", label: "GitLab", says: "a self-managed GitLab group's application" },
] as const;

type ForgeWord = (typeof FORGES)[number]["value"];

/** The words of an answer that is not what the core sends. */
const failure = (err: unknown) => ({ status: "error" as const, error: String(err) });

/** **Use this application**: pin the client id with the owner the person expects. */
function UseApplication({
  onPinned,
  onCancel,
}: {
  onPinned: (said: string) => void;
  onCancel: () => void;
}) {
  const [forge, setForge] = useState<ForgeWord>("github");
  const [host, setHost] = useState("");
  const [clientId, setClientId] = useState("");
  const [owner, setOwner] = useState("");
  const [trouble, setTrouble] = useState<string>();
  const [copied, setCopied] = useState<string>();
  const [busy, setBusy] = useState(false);

  const copyRequest = async () => {
    setCopied(undefined);
    const said = await commands.forgeApplicationRequest(forge, host).catch(failure);
    if (said.status === "error") {
      setCopied(said.error);
      return;
    }
    const put = navigator.clipboard?.writeText(said.data);
    if (!put) {
      setCopied("purlis could not copy the request.");
      return;
    }
    await put.then(
      () => setCopied("The request is on the clipboard. Send it to an owner."),
      () => setCopied("purlis could not copy the request."),
    );
  };

  const pin = async () => {
    if (busy) return;
    setTrouble(undefined);
    setBusy(true);
    const said = await commands.forgeUseApplication(forge, host, clientId, owner).catch(failure);
    setBusy(false);
    if (said.status === "error") {
      setTrouble(said.error);
      return;
    }
    const was = said.data.replaced;
    onPinned(
      said.data.change === "client-id" && was !== null
        ? `${said.data.host} now uses a different application, so sign-ins made through ${was} cannot be renewed. Sign in again.`
        : said.data.change === "owner" && was !== null
          ? `${said.data.host} now expects ${owner.trim()} to own the application, not ${was}. ${said.data.consent}`
          : `${said.data.host} will use this application. ${said.data.consent}`,
    );
  };

  return (
    <Dialog
      onClose={() => {
        if (!busy) onCancel();
      }}
      title="Use this application"
      subtitle="made by an owner of your organization or group, pinned on this machine"
      cancel={{ isDisabled: busy, onPress: onCancel }}
      main={{
        children: busy ? "Pinning…" : "Use this application",
        reserve: ["Use this application", "Pinning…"],
        isDisabled: busy || host.trim() === "" || clientId.trim() === "" || owner.trim() === "",
        onPress: () => void pin(),
      }}
    >
      <SettingRow
        label="Forge"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={FORGES.map((one) => ({ value: one.value, label: one.label, says: one.says }))}
            value={forge}
            onValueChange={(to) => setForge(to === "gitlab" ? "gitlab" : "github")}
            disabled={busy}
          />
        )}
      />
      <SettingRow
        label="Host"
        help="Your server's address."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={host}
            placeholder="ghe.example.com"
            disabled={busy}
            onChange={setHost}
          />
        )}
      />
      <SettingRow
        label="Client ID"
        help="Shown on the application's page. It is public: purlis sends no secret."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={clientId}
            placeholder="Iv1.0123456789abcdef"
            disabled={busy}
            onChange={setClientId}
          />
        )}
      />
      <SettingRow
        label="Owner"
        help="The organization or group you expect to own the application. The forge's page shows its owner when you sign in, and you check that it matches."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={owner}
            placeholder="acme-platform"
            disabled={busy}
            onChange={setOwner}
          />
        )}
      />
      <div className="ui-setting-help ui-text-secondary">
        <p>
          No application yet? An owner makes it once for everyone.{" "}
          <Button
            variant="ghost"
            className="ui-setting-reset"
            isDisabled={host.trim() === ""}
            onPress={() => void copyRequest()}
          >
            Copy a request for an owner
          </Button>
        </p>
        {copied !== undefined && <p role="status">{copied}</p>}
      </div>
      {trouble !== undefined && (
        <p className="trouble" role="alert">
          {trouble}
        </p>
      )}
    </Dialog>
  );
}

/** **Your organization's application**: the pins, each with its owner and a sign-in. */
export const applications: LiveSetting = {
  id: "you.accounts.applications",
  label: "Your organization's application",
  help: "On a GitHub Enterprise Server host or a self-managed GitLab, an owner of an organization or group makes one application for purlis. Pin it here with the owner you expect, then sign in to the host once.",
  writes: "this machine",
  useControl: function useApplications() {
    const [rows, setRows] = useState<ForgeApplicationRow[]>();
    const [trouble, setTrouble] = useState<string>();
    const [said, setSaid] = useState<string>();
    const [adding, setAdding] = useState(false);
    const [signingIn, setSigningIn] = useState<ForgeApplicationRow>();

    const read = useCallback(() => {
      void commands
        .forgeApplications()
        .catch(failure)
        .then((answer) => {
          if (answer.status === "error") setTrouble(answer.error);
          else {
            setTrouble(undefined);
            setRows(Array.isArray(answer.data) ? answer.data : []);
          }
        });
    }, []);
    useEffect(read, [read]);

    const remove = async (row: ForgeApplicationRow) => {
      setSaid(undefined);
      const done = await commands.forgeRemoveApplication(row.kind, row.host).catch(failure);
      if (done.status === "error") setTrouble(done.error);
      else
        setSaid(
          `${row.host} no longer has an application. Accounts signed in through it stay until they end.`,
        );
      read();
    };

    const signedIn = () => {
      setSigningIn(undefined);
      read();
      forgeAccountsChanged();
    };

    return {
      grouped: true,
      error: trouble === undefined ? undefined : [trouble],
      control: (ids) => (
        <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
          {rows === undefined && trouble === undefined && (
            <p className="ui-setting-help ui-text-secondary">Reading…</p>
          )}
          {rows !== undefined &&
            (rows.length === 0 ? (
              <p className="ui-setting-help ui-text-secondary">
                No application is pinned. github.com and gitlab.com need none.
              </p>
            ) : (
              <Table
                className="ui-record-table"
                aria-label="Pinned applications"
                aria-describedby={ids.describedBy}
                stacks
              >
                <TableHeader>
                  <TableColumn id="host" isRowHeader>
                    Host
                  </TableColumn>
                  <TableColumn id="owner">Owner</TableColumn>
                  <TableColumn id="client">Client ID</TableColumn>
                  <TableColumn id="act" align="end" hideName>
                    Sign in or remove
                  </TableColumn>
                </TableHeader>
                <TableBody>
                  {rows.map((row) => (
                    <TableRow
                      key={`${row.kind}:${row.host}`}
                      id={`${row.kind}:${row.host}`}
                      textValue={row.host}
                    >
                      <TableCell>{row.host}</TableCell>
                      <TableCell heading="Owner">
                        {row.owner === "" ? "no owner named" : row.owner}
                        <span className="ui-setting-help ui-text-secondary"> {row.consent}</span>
                      </TableCell>
                      <TableCell heading="Client ID">
                        <span className="ui-record-where ui-text-meta">{row.client_id}</span>
                      </TableCell>
                      <TableCell align="end">
                        {row.signed_in > 0 && <Badge variant="ok">Signed in</Badge>}{" "}
                        <Button
                          size="sm"
                          aria-label={`Sign in to ${row.host}`}
                          onPress={() => setSigningIn(row)}
                        >
                          Sign in
                        </Button>{" "}
                        <Button
                          size="sm"
                          aria-label={`Remove the application for ${row.host}`}
                          onPress={() => void remove(row)}
                        >
                          Remove
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ))}
          {said !== undefined && (
            <p className="ui-setting-help ui-text-secondary" role="status">
              {said}
            </p>
          )}
          <Button onPress={() => setAdding(true)}>Use an application…</Button>
          {adding && (
            <UseApplication
              onCancel={() => setAdding(false)}
              onPinned={(words) => {
                setAdding(false);
                setSaid(words);
                read();
              }}
            />
          )}
          {signingIn !== undefined && signingIn.kind === "github" && (
            <ForgeDeviceSignIn
              host={signingIn.host}
              consent={signingIn.consent}
              onCancel={() => setSigningIn(undefined)}
              onSignedIn={signedIn}
            />
          )}
          {signingIn !== undefined && signingIn.kind === "gitlab" && (
            <ForgeGitLabSignIn
              host={signingIn.host}
              consent={signingIn.consent}
              onCancel={() => setSigningIn(undefined)}
              onSignedIn={signedIn}
            />
          )}
        </div>
      ),
    };
  },
};

/** What a repo's row says of who serves it. */
function servedBy(row: ProjectAccountRow): string {
  if (row.serving !== null) return row.serving.login;
  return "signed out";
}

/** **Account for each project**: which account serves each repo the person confirmed. */
export const projectAccounts: LiveSetting = {
  id: "you.accounts.project-accounts",
  label: "Account for each project",
  help: "A repo is recorded when you clone it from the window or press Trust this clone. With a personal and an enterprise account on one host, choose which one serves each repo. A chat works as that account and no other.",
  writes: "this machine",
  useControl: function useProjectAccounts() {
    const [rows, setRows] = useState<ProjectAccountRow[]>();
    const [trouble, setTrouble] = useState<string>();

    const read = useCallback(() => {
      void commands
        .forgeProjectAccounts()
        .catch(failure)
        .then((answer) => {
          if (answer.status === "error") setTrouble(answer.error);
          else {
            setTrouble(undefined);
            setRows(Array.isArray(answer.data) ? answer.data : []);
          }
        });
    }, []);
    useEffect(read, [read]);

    const choose = async (row: ProjectAccountRow, accountId: string) => {
      const done = await commands
        .forgeChooseProjectAccount(row.clone_key, row.repo_id, accountId)
        .catch(failure);
      if (done.status === "error") setTrouble(done.error);
      read();
    };

    return {
      grouped: true,
      error: trouble === undefined ? undefined : [trouble],
      control: (ids) => (
        <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
          {rows === undefined && trouble === undefined && (
            <p className="ui-setting-help ui-text-secondary">Reading…</p>
          )}
          {rows !== undefined &&
            (rows.length === 0 ? (
              <p className="ui-setting-help ui-text-secondary">
                No repo is recorded yet. Clone one from the window, or press Trust this clone.
              </p>
            ) : (
              <Table
                className="ui-record-table"
                aria-label="Account for each repo"
                aria-describedby={ids.describedBy}
                stacks
              >
                <TableHeader>
                  <TableColumn id="repo" isRowHeader>
                    Repo
                  </TableColumn>
                  <TableColumn id="host">Host</TableColumn>
                  <TableColumn id="account">Served by</TableColumn>
                </TableHeader>
                <TableBody>
                  {rows.map((row) => (
                    <TableRow
                      key={`${row.clone_key}:${row.repo_id}`}
                      id={`${row.clone_key}:${row.repo_id}`}
                      textValue={row.path}
                    >
                      <TableCell>{row.path}</TableCell>
                      <TableCell heading="Host">
                        <span className="ui-record-where ui-text-meta">{row.host}</span>
                      </TableCell>
                      <TableCell heading="Served by">
                        {row.choices.length > 1 ? (
                          <Select
                            size="sm"
                            aria-label={`Account for ${row.path} on ${row.host}`}
                            value={row.serving?.id ?? ""}
                            placeholder="Choose an account"
                            options={row.choices.map((one) => ({
                              value: one.id,
                              label: one.login,
                            }))}
                            onValueChange={(to) => void choose(row, to)}
                          />
                        ) : (
                          <Badge variant={row.chosen_is_signed_out ? "waiting" : "accent"}>
                            {servedBy(row)}
                          </Badge>
                        )}
                        {row.chosen_is_signed_out && (
                          <span className="ui-setting-help ui-text-secondary">
                            {" "}
                            Sign that account in again, or choose another. No other account serves
                            this repo meanwhile.
                          </span>
                        )}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            ))}
        </div>
      ),
    };
  },
};
