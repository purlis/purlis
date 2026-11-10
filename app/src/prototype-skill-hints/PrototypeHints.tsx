// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#5874, branch prototype/skill-hints). Never merges to main.
 *
 * Question: how does the window mark a skill named bare (a hint, ADR 0102 as amended 2026-11-08)
 * beside a reference, on the skill's tab (Uses and used by) and in the install review?
 *   A  A mark on the row     one list; a hint row has a Hint badge, why on its own line, and
 *                            "Write skills:<name>" as a row action. The review says "comes along
 *                            by a hint" and lets the person untick it.
 *   B  Its own group         "Refers to" and "Hints at" apart, with one fix for the whole group
 *                            and the lines it changes. The review lists hinted skills apart,
 *                            unticked until the person ticks them.
 *   C  Quiet, in a tooltip   one list, a hint written "(named bare)" with the fix in its tooltip;
 *                            the fix stays where the lint line already is (the Text tab). The
 *                            review brings it along like a reference, locked.
 * Built from app/src/ui's families on stand-in data; nothing is written.
 */
import { useState } from "react";
import { Badge } from "../ui/badge/Badge";
import { Button, ButtonRow } from "../ui/button/Button";
import { Card, CardTitle } from "../ui/card/Card";
import { Checkbox } from "../ui/choice/Checkbox";
import { Disclosure } from "../ui/disclosure/Disclosure";
import { Fact, Facts } from "../ui/card/Facts";
import { ReviewPage } from "../ui/review-page/ReviewPage";
import { ProtoBar, useParam, type Look } from "./ProtoBar";
import "./prototype.css";

const VARIANTS = [
  { key: "A", name: "A mark on the row", says: "One list. A hint is marked, says why, and offers its fix on the row; the review lets you untick it." },
  { key: "B", name: "Its own group", says: "References and hints apart, one fix for every hint; the review asks you to tick a hinted skill in." },
  { key: "C", name: "Quiet, in a tooltip", says: "One list, a hint named as such with its fix in a tooltip; the review brings it like a reference." },
] as const;

const MOMENTS = [
  { key: "tab", name: "The skill's tab" },
  { key: "review", name: "Install review" },
] as const;

type Named = { name: string; how: "reference" | "hint"; line: number; said: string; project: boolean; broken?: boolean };

/** What `release-train` names, as `skill_details` would send it with `UsesOf::hinted_by`. */
const USES: Named[] = [
  { name: "changelog-fragment", how: "reference", line: 9, said: "skills:changelog-fragment", project: true },
  { name: "notify", how: "reference", line: 21, said: "skills:notify", project: true },
  { name: "grilling", how: "hint", line: 14, said: 'Use the Skill tool with "grilling" before you cut the branch.', project: true },
  { name: "humanizer", how: "hint", line: 30, said: "Run the `humanizer` skill over the release notes.", project: true },
  { name: "old-lint", how: "reference", line: 33, said: "skills:old-lint", project: true, broken: true },
];

const USED_BY: { name: string; how: "reference" | "hint" }[] = [
  { name: "landing-a-pr", how: "reference" },
  { name: "report-intake", how: "hint" },
];

/** What the review of `release-train` from acme/skills brings along. */
const REVIEWED = [
  { name: "release-train", description: "Cut a release branch, write the notes and tag it.", by: null as null | { skill: string; how: "reference" | "hint"; line: string }, risky: 1 },
  { name: "notify", description: "Post a message to the team's channel.", by: { skill: "release-train", how: "reference" as const, line: "skills:notify" }, risky: 1 },
  { name: "changelog-fragment", description: "Write a changes/ fragment for a pull request.", by: { skill: "release-train", how: "hint" as const, line: 'Use the Skill tool with "changelog-fragment"' }, risky: 0 },
];

function Fixed({ fixed }: { fixed: readonly string[] }) {
  return fixed.length === 0 ? null : (
    <p className="proto-said" role="status">
      Written: {fixed.map((one) => `skills:${one}`).join(", ")}. The lint line is gone, and the row is a reference.
    </p>
  );
}

function UsesA({ fixed, onFix }: { fixed: string[]; onFix: (name: string) => void }) {
  return (
    <section aria-labelledby="uses-a">
      <h3 id="uses-a">Uses and used by</h3>
      <ul className="proto-rows" aria-label="Skills release-train names">
        {USES.map((one) => {
          const hint = one.how === "hint" && !fixed.includes(one.name);
          return (
            <li key={one.name}>
              <span className="proto-grow">
                Uses <code>{one.name}</code>
                {one.broken && <span className="honest"> — no skill has this name, so the reference is broken</span>}
                {hint && (
                  <span className="proto-sub honest">
                    Named bare on line {one.line}, so a chat may or may not load it. Write <code>skills:{one.name}</code> to make it a reference.
                  </span>
                )}
              </span>
              {hint && <Badge variant="waiting">Hint</Badge>}
              {hint && (
                <Button size="sm" onPress={() => onFix(one.name)}>
                  Write skills:{one.name}
                </Button>
              )}
            </li>
          );
        })}
        {USED_BY.map((one) => (
          <li key={`by-${one.name}`}>
            <span className="proto-grow">
              Used by <code>{one.name}</code>
              {one.how === "hint" && <span className="proto-sub honest">It names release-train bare. Its own tab offers the fix.</span>}
            </span>
            {one.how === "hint" && <Badge variant="waiting">Hint</Badge>}
          </li>
        ))}
      </ul>
    </section>
  );
}

function UsesB({ fixed, onFix }: { fixed: string[]; onFix: (names: string[]) => void }) {
  const refs = USES.filter((one) => one.how === "reference" || fixed.includes(one.name));
  const hints = USES.filter((one) => one.how === "hint" && !fixed.includes(one.name));
  return (
    <section aria-labelledby="uses-b" className="proto-groups">
      <h3 id="uses-b">Uses and used by</h3>
      <Card>
        <CardTitle as="h4">Refers to ({refs.length})</CardTitle>
        <ul className="proto-rows">
          {refs.map((one) => (
            <li key={one.name}>
              <code className="proto-grow">{one.name}</code>
              {one.broken && <Badge variant="waiting">Broken</Badge>}
            </li>
          ))}
        </ul>
      </Card>
      {hints.length > 0 && (
        <Card>
          <CardTitle as="h4">Hints at ({hints.length})</CardTitle>
          <p className="honest">
            Named bare beside the word skill. A chat reads them as a hint and may not load them. A reference is <code>skills:&lt;name&gt;</code>.
          </p>
          <ul className="proto-rows">
            {hints.map((one) => (
              <li key={one.name}>
                <code>{one.name}</code>
                <span className="proto-grow honest">
                  line {one.line}: {one.said}
                </span>
              </li>
            ))}
          </ul>
          <ButtonRow>
            <Button size="sm" variant="primary" onPress={() => onFix(hints.map((one) => one.name))}>
              Write them as references
            </Button>
          </ButtonRow>
        </Card>
      )}
      <Card>
        <CardTitle as="h4">Used by</CardTitle>
        <ul className="proto-rows">
          <li>
            <code className="proto-grow">landing-a-pr</code>
            <span className="honest">refers to it</span>
          </li>
          <li>
            <code className="proto-grow">report-intake</code>
            <span className="honest">hints at it</span>
          </li>
        </ul>
      </Card>
    </section>
  );
}

function UsesC({ fixed }: { fixed: string[] }) {
  return (
    <section aria-labelledby="uses-c">
      <h3 id="uses-c">Uses and used by</h3>
      <ul className="proto-rows" aria-label="Skills release-train names">
        {USES.map((one) => {
          const hint = one.how === "hint" && !fixed.includes(one.name);
          return (
            <li key={one.name}>
              <span className="proto-grow">
                Uses <code>{one.name}</code>
                {one.broken && <span className="honest"> — no skill has this name, so the reference is broken</span>}
              </span>
              {hint && (
                <Button
                  size="sm"
                  variant="quiet"
                  tooltip={`Named bare on line ${one.line}: a hint, which a chat may not follow. Write skills:${one.name} on the Text tab to make it a reference.`}
                >
                  named bare
                </Button>
              )}
            </li>
          );
        })}
        <li>
          <span className="proto-grow">
            Used by <code>landing-a-pr</code>
          </span>
        </li>
        <li>
          <span className="proto-grow">
            Used by <code>report-intake</code>
          </span>
          <Button size="sm" variant="quiet" tooltip="report-intake names release-train bare: a hint.">
            named bare
          </Button>
        </li>
      </ul>
      <p className="honest">The fix stays where the lint line already is: the Text tab marks line 14 and line 30.</p>
    </section>
  );
}

function SkillTab({ variant }: { variant: string }) {
  const [fixed, setFixed] = useState<string[]>([]);
  return (
    <div className="proto-main">
      <div className="proto-inline">
        <h2 className="proto-title">
          <code>release-train</code>
        </h2>
        <Badge variant="ok">Project</Badge>
        <span className="proto-grow" />
        <ButtonRow>
          <Button size="sm" variant="primary">Use in this chat</Button>
          <Button size="sm">Edit</Button>
          <Button size="sm">Grade</Button>
        </ButtonRow>
      </div>
      <ButtonRow>
        {["Overview", "Text", "History", "Grades", "Runs"].map((tab) => (
          <Button key={tab} size="sm" variant={tab === "Overview" ? "soft-accent" : "ghost"}>
            {tab}
          </Button>
        ))}
      </ButtonRow>
      <Facts>
        <Fact label="Description">Cut a release branch, write the notes and tag it.</Fact>
        <Fact label="Level">Project, in .claude/skills/release-train</Fact>
        <Fact label="Checks">2 warnings: grilling and humanizer are named bare</Fact>
      </Facts>
      {variant === "A" && <UsesA fixed={fixed} onFix={(name) => setFixed([...fixed, name])} />}
      {variant === "B" && <UsesB fixed={fixed} onFix={(names) => setFixed([...fixed, ...names])} />}
      {variant === "C" && <UsesC fixed={fixed} />}
      <Fixed fixed={fixed} />
    </div>
  );
}

function Review({ variant }: { variant: string }) {
  const [kept, setKept] = useState<Record<string, boolean>>({ "changelog-fragment": variant !== "B" });
  const [done, setDone] = useState<string>();
  const brought = REVIEWED.filter((one) => variant !== "B" || one.by?.how !== "hint");
  const hinted = REVIEWED.filter((one) => one.by?.how === "hint");
  const count = REVIEWED.filter((one) => one.by?.how !== "hint" || kept[one.name]).length;
  const row = (one: (typeof REVIEWED)[number]) => {
    const hint = one.by?.how === "hint";
    const locked = one.by !== null && (!hint || variant === "C");
    return (
      <section key={one.name} className="proto-skill" aria-label={`Review of ${one.name}`}>
        <Checkbox
          isSelected={!hint || kept[one.name] === true}
          isDisabled={locked || one.by === null}
          onChange={(on) => setKept({ ...kept, [one.name]: on })}
          tooltip={
            locked
              ? hint
                ? `It comes along while ${one.by?.skill} is installed.`
                : `It comes along while ${one.by?.skill} is installed.`
              : undefined
          }
          description={
            hint && variant === "A"
              ? `${one.by?.skill} only names it bare, so it runs without it. Untick it to leave it out.`
              : hint && variant === "B"
                ? `Tick it if you want ${one.by?.skill}'s hint to find it here.`
                : undefined
          }
        >
          Install {one.name}
          {one.risky > 0 && ` (${one.risky} risky item)`}
        </Checkbox>
        <h3 className="ui-text-strong">
          <code>{one.name}</code> {one.description}
        </h3>
        {one.by !== null && (
          <p className="honest">
            {one.by.how === "reference"
              ? `Comes along: ${one.by.skill} refers to it.`
              : variant === "A"
                ? `Comes along by a hint: ${one.by.skill} names it bare ("${one.by.line}").`
                : variant === "B"
                  ? `${one.by.skill} names it bare ("${one.by.line}"): a hint, not a reference.`
                  : `Comes along: ${one.by.skill} hints at it.`}
          </p>
        )}
        <Disclosure size="sm" label={`Its files (${one.name === "release-train" ? 3 : 1})`} trail={one.risky ? <Badge variant="waiting">Look closely</Badge> : <Badge variant="ok">Plain</Badge>}>
          <pre className="proto-said">SKILL.md{one.name === "release-train" ? "\nscripts/tag.sh\nnotes.md" : ""}</pre>
        </Disclosure>
      </section>
    );
  };
  return (
    <ReviewPage
      label="Install release-train from acme/skills"
      summary={done ?? `Nothing is installed until you press Install. ${count} skills, pinned to 4be9c21.`}
      onCancel={() => setDone("Cancelled.")}
      main={{ children: "Install", onPress: () => setDone(`Installed ${count} skills.`) }}
    >
      <div className="proto-main">
        <p>
          You are reviewing <code>release-train</code> from <code>github.com/acme/skills</code> at commit <code>4be9c21</code>. What you install stays pinned to it.
        </p>
        {brought.map(row)}
        {variant === "B" && (
          <Card>
            <CardTitle as="h4">Hinted at, not referred to</CardTitle>
            <p className="honest">release-train names these bare. They are not needed for it to run, so they are left out unless you tick them.</p>
            {hinted.map(row)}
          </Card>
        )}
      </div>
    </ReviewPage>
  );
}

export function PrototypeHints({ look, onLook }: { look?: Look; onLook?: (to: Look) => void }) {
  const [variant, setVariant] = useParam("variant", VARIANTS.map((one) => one.key));
  const [moment, setMoment] = useParam("moment", MOMENTS.map((one) => one.key));
  return (
    <div className="proto-page">
      <div className="proto-stage">
        <section className="proto-window" aria-label={moment === "tab" ? "The skill's tab" : "The install review"} key={`${variant}-${moment}`}>
          {moment === "tab" ? <SkillTab variant={variant} /> : <Review variant={variant} />}
        </section>
        <section className="proto-notes">
          <h1>A skill named bare, as a hint (#5874)</h1>
          <p className="honest">
            Prototype on stand-in data, drawn from the real families. Press the fixes and the
            ticks: nothing is written.
          </p>
          <h2>The three questions</h2>
          <ul>
            <li>How a hint looks next to a reference: {variant === "A" ? "a Hint mark on the row, and why under it." : variant === "B" ? "a group of its own, Hints at." : "the words named bare, and why in a tooltip."}</li>
            <li>Whether the tab offers the fix: {variant === "A" ? "yes, one per row." : variant === "B" ? "yes, one for the whole group." : "no; the Text tab's lint line keeps it."}</li>
            <li>How the review words it, and untick: {variant === "A" ? "\"Comes along by a hint\", ticked, can be unticked." : variant === "B" ? "listed apart, unticked until you tick it." : "\"hints at it\", ticked and locked, as a reference is."}</li>
          </ul>
          <h2>Where the words come from</h2>
          <p className="honest">
            The lines are what <code>purlis skill uses</code> and the skill lint already say:
            a skill quoted or in backticks, on a line that says skill, where a skill of that name exists.
          </p>
        </section>
      </div>
      <ProtoBar variants={VARIANTS} variant={variant} onVariant={setVariant} moments={MOMENTS} moment={moment} onMoment={setMoment} look={look} onLook={onLook} />
    </div>
  );
}
