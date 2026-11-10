// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#6031, branch prototype/skill-policy). Never merges to main.
 *
 * Question: where does the window show an org's `[skill_policy]` (allow and block lists,
 * `min_age_days`, `min_age_exempt`) and its effect: what a source in Sources would be refused
 * before it is reviewed, an update too young to take (and when it can be), and the audited acts?
 *   A  At the point of refusal  each source, skill and update says what the controls do to it,
 *                               on its own row; one line on Sources says controls apply and links
 *                               to Settings, which holds the table and the recorded acts.
 *   B  A Controls page          a page of its own in the studio's navigation: the rules, what
 *                               they refuse now, what waits to be old enough, and the recorded
 *                               acts. Rows elsewhere carry a Refused mark that opens it.
 *   C  Only what is allowed     Sources and Updates list what the controls admit; what they refuse
 *                               is folded under one line ("3 left out by this project's
 *                               controls"); Settings holds the table and the acts.
 * Built from app/src/ui's families and Settings' six pieces on stand-in data; nothing is changed.
 */
import { useState } from "react";
import { Badge } from "../ui/badge/Badge";
import { Button, ButtonRow } from "../ui/button/Button";
import { Card, CardTitle } from "../ui/card/Card";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { Fact, Facts } from "../ui/card/Facts";
import { Table, TableBody, TableCell, TableColumn, TableHeader, TableRow } from "../ui/table/Table";
import { Notice } from "../Notice";
import { SettingGroup, SettingRow } from "../settings/components";
import { ProtoBar, useParam, type Look } from "./ProtoBar";
import "./prototype.css";

const VARIANTS = [
  { key: "A", name: "At the point of refusal", says: "Each row says what the controls do to it; Settings holds the table and the record." },
  { key: "B", name: "A Controls page", says: "A studio page of its own: the rules, what they refuse, what waits, and the record." },
  { key: "C", name: "Only what is allowed", says: "Sources and Updates list what is allowed; what is refused folds under one line." },
] as const;

const MOMENTS = [
  { key: "sources", name: "Sources" },
  { key: "updates", name: "Updates" },
  { key: "settings", name: "Settings" },
  { key: "record", name: "Recorded acts" },
] as const;

const SHARED = "charter.toml";

/** The project's controls, as the shared file writes them. */
const POLICY = {
  allow: [
    { source: "https://github.com/acme/skills" },
    { source: "https://github.com/anthropics/skills", skill: "pdf" },
    { extension: "acme.registry" },
  ],
  block: [{ source: "https://github.com/acme/skills", skill: "yolo" }],
  min_age_days: 7,
  min_age_exempt: ["9f3c1d27a0b4e5f6a7b8c9d0e1f2a3b4c5d6e7f8"],
};

type Verdict = { ok: true } | { ok: false; why: string; short: string };

const SOURCES: { url: string; skills: number; installed: number; checked: string; verdict: Verdict }[] = [
  { url: "github.com/acme/skills", skills: 14, installed: 6, checked: "2 hours ago", verdict: { ok: true } },
  { url: "github.com/anthropics/skills", skills: 22, installed: 1, checked: "yesterday", verdict: { ok: false, short: "Only pdf allowed", why: "Only pdf is on this project's allow list: the other 21 skills of github.com/anthropics/skills are not." } },
  { url: "github.com/obra/superpowers", skills: 31, installed: 0, checked: "3 days ago", verdict: { ok: false, short: "Not on the allow list", why: "github.com/obra/superpowers is not on this project's allow list of skill sources." } },
];

const SKILLS_OF_ACME: { name: string; state: string; verdict: Verdict }[] = [
  { name: "release-train", state: "Installed", verdict: { ok: true } },
  { name: "notify", state: "Installed", verdict: { ok: true } },
  { name: "yolo", state: "Not installed", verdict: { ok: false, short: "Blocked", why: "yolo from github.com/acme/skills is on this project's block list." } },
  { name: "db-migrate", state: "Not installed", verdict: { ok: true } },
];

const UPDATES: { name: string; from: string; to: string; committed: string; verdict: Verdict; on?: string }[] = [
  { name: "release-train", from: "4be9c21", to: "a71d0e3", committed: "11 days ago", verdict: { ok: true } },
  { name: "notify", from: "4be9c21", to: "c08f5b2", committed: "4 days ago", verdict: { ok: false, short: "Too young, 3 days to go", why: "c08f5b2 was committed 4 days ago, and this project installs only commits at least 7 days old: it can be installed in 3 days." }, on: "Thursday 13 November" },
  { name: "pdf", from: "e11b0aa", to: "5d2c9f4", committed: "1 day ago", verdict: { ok: false, short: "Too young, 6 days to go", why: "5d2c9f4 was committed 1 day ago, and this project installs only commits at least 7 days old: it can be installed in 6 days." }, on: "Sunday 16 November" },
];

const RECORD = [
  { when: "Today 14:02", act: "Install", what: "release-train, notify", source: "acme/skills @ 4be9c21", level: "Project", who: "the operator" },
  { when: "Yesterday 10:40", act: "Refused", what: "superpowers", source: "obra/superpowers", level: "Workspace smart-ide", who: "a chat as steward" },
  { when: "5 Nov 17:15", act: "Update", what: "pdf", source: "anthropics/skills @ e11b0aa", level: "Project", who: "the operator" },
  { when: "2 Nov 09:03", act: "Remove", what: "lint-fix", source: "acme/skills @ 1aa20f9", level: "Persona reviewer", who: "the operator" },
];

function Refused({ verdict, onOpen }: { verdict: Verdict; onOpen?: () => void }) {
  if (verdict.ok) return null;
  return onOpen ? (
    <Button size="sm" variant="warning-soft" tooltip={verdict.why} onPress={onOpen}>
      Refused
    </Button>
  ) : (
    <Badge variant="waiting">{verdict.short}</Badge>
  );
}

function Rules() {
  return (
    <Facts>
      <Fact label="Allowed">
        <code>acme/skills</code> (every skill), <code>anthropics/skills</code> (pdf only), extension <code>acme.registry</code>
      </Fact>
      <Fact label="Blocked">
        <code>yolo</code> from <code>acme/skills</code>
      </Fact>
      <Fact label="Minimum age">7 days from the commit</Fact>
      <Fact label="Exempt">
        <code>9f3c1d2</code>, installed younger by a reviewed change
      </Fact>
      <Fact label="Set in">
        <code>{SHARED}</code>, <code>[skill_policy]</code>: the team&apos;s shared file. This machine&apos;s file cannot loosen it.
      </Fact>
    </Facts>
  );
}

function RecordTable() {
  return (
    <Table aria-label="Recorded skill acts">
      <TableHeader>
        <TableColumn isRowHeader>When</TableColumn>
        <TableColumn>Act</TableColumn>
        <TableColumn>Skills</TableColumn>
        <TableColumn>Source</TableColumn>
        <TableColumn>Level</TableColumn>
        <TableColumn>By</TableColumn>
      </TableHeader>
      <TableBody>
        {RECORD.map((one) => (
          <TableRow key={one.when} id={one.when} textValue={`${one.act} ${one.what}`}>
            <TableCell>{one.when}</TableCell>
            <TableCell>{one.act === "Refused" ? <Badge variant="waiting">Refused</Badge> : one.act}</TableCell>
            <TableCell>{one.what}</TableCell>
            <TableCell>{one.source}</TableCell>
            <TableCell>{one.level}</TableCell>
            <TableCell>{one.who}</TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
}

function SettingsPage({ variant, onControls }: { variant: string; onControls: () => void }) {
  const row = (label: string, value: string, help: string) => (
    <SettingRow label={label} help={help} origin={`Set in ${SHARED}`} control={(ids) => <code id={ids.id}>{value}</code>} />
  );
  return (
    <div className="proto-main">
      <SettingGroup label="Skill sources: this project's controls" kept={`Kept in ${SHARED}, which your team sees. Read only here: change it by a reviewed change to that file.`}>
        {row("Allow", "acme/skills · anthropics/skills (pdf) · acme.registry", "Only these sources install. Absent, every source not blocked does.")}
        {row("Block", "acme/skills (yolo)", "Never installed. A block wins over an allow.")}
        {row("Minimum age", "7 days", "A commit installs or updates only once it is this old. 0 turns it off.")}
        {row("Exempt commits", "9f3c1d2", "Each installs younger, by a reviewed change.")}
      </SettingGroup>
      {variant === "B" ? (
        <ButtonRow>
          <Button size="sm" onPress={onControls}>
            Open Controls in the Skill studio
          </Button>
        </ButtonRow>
      ) : (
        <SettingGroup label="Recorded skill acts" help="Every install, update and removal, and every refusal, as the session host recorded it.">
          <RecordTable />
        </SettingGroup>
      )}
    </div>
  );
}

function Sources({ variant, onControls }: { variant: string; onControls: () => void }) {
  const allowed = SOURCES.filter((one) => one.verdict.ok);
  const refused = SOURCES.filter((one) => !one.verdict.ok);
  const sourceRow = (one: (typeof SOURCES)[number]) => (
    <li key={one.url}>
      <span className="proto-grow">
        <code>{one.url}</code>
        <span className="proto-sub honest">
          {one.skills} skills, {one.installed} installed · checked {one.checked}
        </span>
        {variant === "A" && !one.verdict.ok && <span className="proto-sub honest">{one.verdict.why}</span>}
      </span>
      {variant === "A" && <Refused verdict={one.verdict} />}
      {variant === "B" && <Refused verdict={one.verdict} onOpen={onControls} />}
    </li>
  );
  const skillRow = (one: (typeof SKILLS_OF_ACME)[number]) => (
    <li key={one.name}>
      <code className="proto-grow">{one.name}</code>
      <span className="honest">{one.state}</span>
      {variant === "B" ? <Refused verdict={one.verdict} onOpen={onControls} /> : <Refused verdict={one.verdict} />}
      {one.state === "Not installed" && (
        <Button size="sm" isDisabled={!one.verdict.ok} tooltip={one.verdict.ok ? undefined : one.verdict.why}>
          Review install
        </Button>
      )}
    </li>
  );
  return (
    <div className="proto-main">
      <div className="proto-inline">
        <h2 className="proto-title proto-grow">Sources</h2>
        <Button size="sm">Add source…</Button>
      </div>
      {variant === "A" && (
        <Notice cause="skill-policy-applies" link={{ label: "See the controls", onPress: onControls }}>
          This project&apos;s controls apply: 2 sources allowed, 1 skill blocked, commits at least 7 days old.
        </Notice>
      )}
      <h3>Your sources</h3>
      <ul className="proto-rows">{(variant === "C" ? allowed : SOURCES).map(sourceRow)}</ul>
      {variant === "C" && (
        <Disclosure size="sm" label={`${refused.length} sources left out by this project's controls`}>
          <ul className="proto-rows">
            {refused.map((one) => (
              <li key={one.url}>
                <code className="proto-grow">{one.url}</code>
                <span className="honest">{!one.verdict.ok && one.verdict.why}</span>
              </li>
            ))}
          </ul>
        </Disclosure>
      )}
      <h3>acme/skills</h3>
      <ul className="proto-rows">{(variant === "C" ? SKILLS_OF_ACME.filter((one) => one.verdict.ok) : SKILLS_OF_ACME).map(skillRow)}</ul>
      {variant === "C" && (
        <Disclosure size="sm" label="1 skill left out by this project's controls">
          <p className="honest">yolo is on this project&apos;s block list.</p>
        </Disclosure>
      )}
    </div>
  );
}

function Updates({ variant, onControls }: { variant: string; onControls: () => void }) {
  const ready = UPDATES.filter((one) => one.verdict.ok);
  const waiting = UPDATES.filter((one) => !one.verdict.ok);
  const row = (one: (typeof UPDATES)[number]) => (
    <li key={one.name}>
      <span className="proto-grow">
        <code>{one.name}</code> {one.from} → {one.to}
        <span className="proto-sub honest">
          committed {one.committed}
          {!one.verdict.ok && variant !== "B" && `, can be taken on ${one.on}`}
        </span>
      </span>
      {variant === "B" ? <Refused verdict={one.verdict} onOpen={onControls} /> : <Refused verdict={one.verdict} />}
      <Button size="sm" isDisabled={!one.verdict.ok} tooltip={one.verdict.ok ? undefined : one.verdict.why}>
        Review update
      </Button>
    </li>
  );
  return (
    <div className="proto-main">
      <h2 className="proto-title">Updates</h2>
      <p className="honest">Nothing changes until you accept it.</p>
      <ul className="proto-rows">{(variant === "C" ? ready : UPDATES).map(row)}</ul>
      {variant === "C" && (
        <Disclosure size="sm" label={`${waiting.length} updates not old enough yet`}>
          <ul className="proto-rows">{waiting.map(row)}</ul>
        </Disclosure>
      )}
    </div>
  );
}

function ControlsPage() {
  return (
    <div className="proto-main">
      <h2 className="proto-title">Controls</h2>
      <p className="honest">This project is an org&apos;s: what it installs and updates is held to these, and every act is recorded.</p>
      <Card>
        <CardTitle as="h3">The rules</CardTitle>
        <Rules />
      </Card>
      <Card>
        <CardTitle as="h3">What they refuse now</CardTitle>
        <ul className="proto-rows">
          {[...SOURCES, ...SKILLS_OF_ACME.map((one) => ({ url: `${one.name} from acme/skills`, verdict: one.verdict }))]
            .filter((one) => !one.verdict.ok)
            .map((one) => (
              <li key={one.url}>
                <code>{one.url}</code>
                <span className="proto-grow honest">{!one.verdict.ok && one.verdict.why}</span>
              </li>
            ))}
        </ul>
      </Card>
      <Card>
        <CardTitle as="h3">Waiting to be old enough</CardTitle>
        <ul className="proto-rows">
          {UPDATES.filter((one) => !one.verdict.ok).map((one) => (
            <li key={one.name}>
              <code className="proto-grow">
                {one.name} {one.to}
              </code>
              <span className="honest">can be taken on {one.on}</span>
            </li>
          ))}
        </ul>
      </Card>
      <Card>
        <CardTitle as="h3">Recorded acts</CardTitle>
        <RecordTable />
      </Card>
    </div>
  );
}

export function PrototypePolicy({ look, onLook }: { look?: Look; onLook?: (to: Look) => void }) {
  const [variant, setVariant] = useParam("variant", VARIANTS.map((one) => one.key));
  const [moment, setMoment] = useParam("moment", MOMENTS.map((one) => one.key));
  const [controls, setControls] = useState(false);
  const page = controls && variant === "B" ? "controls" : moment;
  const go = (to: string) => {
    setControls(false);
    setMoment(to);
  };
  const toControls = () => (variant === "B" ? setControls(true) : go("settings"));
  const inSettings = page === "settings" || (page === "record" && variant !== "B");
  return (
    <div className="proto-page">
      <div className="proto-stage">
        <section className="proto-window" aria-label={inSettings ? "Settings" : "The Skill studio"}>
          <div className="proto-head">
            <h2>{inSettings ? "Settings · Project" : "Skill studio"}</h2>
          </div>
          <div className="proto-body">
            <nav className="proto-nav" aria-label="Pages">
              {inSettings
                ? ["General", "Chats", "Skill sources", "Sandbox", "Forges"].map((one) => (
                    <Button key={one} size="sm" variant={one === "Skill sources" ? "soft-accent" : "ghost"}>
                      {one}
                    </Button>
                  ))
                : [
                    ["Library", ""],
                    ["Needs you", ""],
                    ["Sources", "sources"],
                    ["Updates", "updates"],
                    ...(variant === "B" ? [["Controls", "controls"]] : []),
                  ].map(([label, to]) => (
                    <Button
                      key={label}
                      size="sm"
                      variant={(to === "controls" ? page === "controls" : to === page) ? "soft-accent" : "ghost"}
                      onPress={() => (to === "controls" ? setControls(true) : to ? go(to) : undefined)}
                    >
                      {label}
                    </Button>
                  ))}
            </nav>
            {page === "controls" || (page === "record" && variant === "B") ? (
              <ControlsPage />
            ) : page === "sources" ? (
              <Sources variant={variant} onControls={toControls} />
            ) : page === "updates" ? (
              <Updates variant={variant} onControls={toControls} />
            ) : (
              <SettingsPage variant={variant} onControls={() => setControls(true)} />
            )}
          </div>
        </section>
        <section className="proto-notes">
          <h1>An org&apos;s skill controls (#6031)</h1>
          <p className="honest">
            Prototype on stand-in data, drawn from the real families and Settings&apos; pieces. The
            controls are the ones <code>[skill_policy]</code> holds today; the refusals are the
            core&apos;s own sentences. Nothing is changed.
          </p>
          <h2>The table, as {SHARED} writes it</h2>
          <pre className="proto-said">{`[skill_policy]
allow = [
  { source = "${POLICY.allow[0]!.source}" },
  { source = "${POLICY.allow[1]!.source}", skill = "pdf" },
  { extension = "acme.registry" },
]
block = [{ source = "${POLICY.block[0]!.source}", skill = "yolo" }]
min_age_days = ${POLICY.min_age_days}
min_age_exempt = ["9f3c1d2…"]`}</pre>
          <h2>Where each acceptance line lands</h2>
          <ul>
            <li>Sources say, before a review, what is refused: {variant === "A" ? "on the row, with the reason." : variant === "B" ? "a Refused mark whose tooltip says why, opening Controls." : "they do not list it; one folded line holds it."}</li>
            <li>Updates say when a young one can be taken: {variant === "A" ? "on the row, with its date." : variant === "B" ? "on the row as Refused, and on Controls under Waiting." : "folded under Not old enough yet, each with its date."}</li>
            <li>Settings shows the table read only, with where it is set; {variant === "B" ? "the record is on Controls." : "the record is beside it."}</li>
          </ul>
        </section>
      </div>
      <ProtoBar variants={VARIANTS} variant={variant} onVariant={(to) => { setControls(false); setVariant(to); }} moments={MOMENTS} moment={moment} onMoment={go} look={look} onLook={onLook} />
    </div>
  );
}
