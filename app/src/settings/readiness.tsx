// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useRef, useState } from "react";
import { commands, type PlaneId, type ReadinessAnswer, type ReadinessCheck } from "../bindings";
import type { LiveSetting, SettingsGroup } from "./groups";
import { Badge } from "../ui/badge/Badge";
import { Button } from "../ui/button/Button";
import { CodeBlock } from "../ui/code/CodeBlock";
import { Disclosure } from "../ui/disclosure/Disclosure";

/**
 * **You › Ready for all users** (one login, slice D): whether the open project's forge would let
 * everyone else with access to it sign in to purlis and work. The checks are the core's
 * (`plane_readiness`, `forge/readiness.rs`): the account's role, the organization's approval of
 * purlis's OAuth app, purlis registered on a self-hosted server, the default branch's protection.
 *
 * Each says **Ready**, **Blocked**, or **Not read**, and a check that could not be read is never
 * a ready one: the host is "ready for all users" only when every check is. A blocker says who
 * can fix it (you, the owner, the admin), the clicks with their addresses, and, when it is not
 * you, a request to send them, ready to copy.
 *
 * Nothing is kept. The group reads when it is shown (which is when a project is opened here)
 * and again on Check again.
 */
export function readinessGroup(plane: PlaneId): SettingsGroup {
  return {
    id: "you.readiness",
    label: "Ready for all users",
    help: "Whether this project's forge would let everyone with access to it sign in to purlis and work, and who can fix what would stop them.",
    settings: [readiness(plane)],
  };
}

type Read =
  | { state: "reading" }
  | { state: "trouble"; trouble: string }
  | { state: "read"; answer: ReadinessAnswer };

const SAID_VERDICT = { ready: "Ready", blocked: "Blocked", unknown: "Not read" } as const;
const WHO = {
  you: "You can fix this.",
  owner: "The owner can fix this.",
  admin: "The administrator can fix this.",
} as const;

function readiness(plane: PlaneId): LiveSetting {
  return {
    id: "you.readiness.project",
    label: "This project",
    help: "Read from the forge as you. A setting purlis cannot read counts as not ready.",
    writes: "nothing",
    useControl: function useReadiness() {
      const [read, setRead] = useState<Read>({ state: "reading" });
      const latest = useRef(0);
      const check = useCallback(async () => {
        const mine = ++latest.current;
        setRead({ state: "reading" });
        const said = await commands
          .planeReadiness(plane)
          .catch((err: unknown) => ({ status: "error" as const, error: String(err) }));
        if (mine !== latest.current) return;
        setRead(
          said.status === "error"
            ? { state: "trouble", trouble: said.error }
            : { state: "read", answer: said.data },
        );
      }, []);
      useEffect(() => {
        void check();
      }, [check]);
      return {
        grouped: true,
        control: (ids) => (
          <div role="group" aria-labelledby={ids.labelledBy} aria-describedby={ids.describedBy}>
            <Verdict read={read} />
            <Button onPress={() => void check()} isDisabled={read.state === "reading"}>
              Check again
            </Button>
          </div>
        ),
      };
    },
  };
}

function Verdict({ read }: { read: Read }) {
  if (read.state === "reading")
    return <p className="ui-setting-help ui-text-secondary">Checking the forge…</p>;
  if (read.state === "trouble")
    return (
      <p className="ui-setting-help ui-text-secondary" role="alert">
        {read.trouble}
      </p>
    );
  const answer = read.answer;
  if (answer.state === "notChecked")
    return (
      <p className="ui-setting-help ui-text-secondary" role="status">
        Not checked. {answer.why}
      </p>
    );
  return (
    <>
      <p role="status">
        <Badge variant={answer.ready ? "ok" : "waiting"}>
          {answer.ready ? "Ready for all users" : "Not ready for all users"}
        </Badge>{" "}
        <span className="ui-record-where ui-text-meta">
          {answer.path} on {answer.host}
        </span>
      </p>
      <ul aria-label="Checks">
        {answer.checks.map((check) => (
          <li key={check.id}>
            <CheckRow one={check} />
          </li>
        ))}
      </ul>
    </>
  );
}

function CheckRow({ one }: { one: ReadinessCheck }) {
  const verdict = SAID_VERDICT[one.verdict as keyof typeof SAID_VERDICT] ?? "Not read";
  const who = one.who === null ? undefined : WHO[one.who as keyof typeof WHO];
  const head = (
    <>
      <Badge variant={one.verdict === "ready" ? "ok" : "waiting"}>{verdict}</Badge> {one.title}
    </>
  );
  if (one.verdict === "ready")
    return (
      <p>
        {head}
        <span className="ui-setting-help ui-text-secondary"> {one.detail}</span>
      </p>
    );
  return (
    <Disclosure label={<>{head}</>} defaultExpanded={one.verdict === "blocked"}>
      <p className="ui-setting-help ui-text-secondary">
        {one.detail}
        {who === undefined ? "" : ` ${who}`}
      </p>
      {one.steps.length > 0 && (
        <ol aria-label={`Steps for ${one.title}`}>
          {one.steps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
      )}
      {one.request !== null && (
        <CodeBlock label="Request to send" text={one.request} copy="Copy request" />
      )}
    </Disclosure>
  );
}
