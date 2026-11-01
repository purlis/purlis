// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway (#4477, branch prototype/skills-plugin-off). Never merges to main. */
import { useState } from "react";
import { Plus, Sparkle, Sparkles } from "lucide-react";
import { Badge } from "../ui/badge/Badge";
import { Button, ButtonRow } from "../ui/button/Button";
import { List } from "../ui/list/List";
import { ListCardRow } from "../ui/list/CardRow";
import { ListFoldSection, ListRun } from "../ui/list/Fold";
import { ListFilter } from "../ui/list/ListFilter";
import { SkillMenu } from "../SkillsList";
import { cardReasonOf, fixOf, markOf, type ListedSkill } from "../skillsListing";
import { STATES, WRITINGS, shownIn, typedBy, type StateId, type WritingId } from "./data";
import "./prototype.css";

export type Look = { appearance: string; theme: string };

export function PrototypeSkills({
  look,
  onLook,
}: {
  look?: Look;
  onLook?: (to: Look) => void;
}) {
  const [stateId, setState] = useState<StateId>("disabled");
  const [writing, setWriting] = useState<WritingId>("reason-and-fix");
  const [query, setQuery] = useState("");
  const [typed, setTyped] = useState<string>();
  const [did, setDid] = useState<string>();
  const state = STATES.find((one) => one.id === stateId) ?? STATES[0]!;
  const shown = shownIn(state, writing);
  const matches = (row: ListedSkill) => row.skill.name.includes(query.trim().toLowerCase());
  const inChat = shown.inChat.filter(matches);
  const builtIns = shown.builtIns.filter(matches);
  const hidden = shown.hidden.filter(matches);
  const off = shown.off.filter(matches);
  const filtering = query.trim() !== "";
  const copy = (command: string) => {
    void navigator.clipboard?.writeText(command).catch(() => undefined);
    setDid(`Copied: ${command}`);
  };
  const card = (row: ListedSkill) => {
    const mark = markOf(row);
    const usable = mark !== "left-out" && mark !== "off" && mark !== "needs-review";
    const isOff = row.inChat === false;
    // Way C says why once, on the plugin's line and its closing line.
    const reason = isOff && writing === "one-line" ? undefined : cardReasonOf(row);
    const fix = isOff ? undefined : fixOf(row, "git-ci-cd");
    return (
      <ListCardRow
        key={row.key}
        id={row.key}
        textValue={row.skill.name}
        name={row.skill.name}
        mark={<Badge>{row.skill.label}</Badge>}
        actions={
          usable ? (
            <Button
              size="sm"
              variant="ghost"
              aria-label={`Use ${row.skill.name} in this chat`}
              onPress={() => {
                setTyped(typedBy(row));
                setDid(undefined);
              }}
            >
              Use
            </Button>
          ) : undefined
        }
        description={reason === undefined ? row.skill.description : undefined}
        reason={reason}
        fix={
          isOff && writing === "reason-and-fix" && state.command !== undefined ? (
            <Button size="sm" onPress={() => copy(state.command!)}>
              Copy command
            </Button>
          ) : fix !== undefined ? (
            <Button size="sm" onPress={() => setDid(fix.label)}>
              {fix.label}
            </Button>
          ) : undefined
        }
        menu={() => (
          <SkillMenu
            row={row}
            at={{ workspace: "git-ci-cd", chat: true, explorer: "git-ci-cd", pinsRefused: null }}
            busy={false}
            onAct={(what) => setDid(`Chose ${what}`)}
          />
        )}
      />
    );
  };
  const count = inChat.length + builtIns.length;
  const leftOut = hidden.length + off.length;
  return (
    <div className="proto-page">
      <div className="proto-stage">
        <aside className="proto-sidebar" aria-label="The Skills view, as the sidebar draws it">
          <div className="skills-view">
            <p className="skills-view-whose">
              <strong>git-ci-cd 1</strong>{" "}
              <span className="honest">· {count} skills</span>
            </p>
            <p className="skills-view-said">{shown.said.join(" · ")}</p>
            <ButtonRow className="skills-view-head">
              <Button variant="ghost" icon={<Sparkles aria-hidden="true" />}>
                Open Skill studio
              </Button>
              <Button variant="ghost" icon={<Plus aria-hidden="true" />}>
                New skill…
              </Button>
            </ButtonRow>
            <ListFilter
              noun={{ one: "skill", many: "skills" }}
              total={shown.inChat.length + shown.builtIns.length + shown.hidden.length + shown.off.length}
              query={query}
              onQuery={setQuery}
            />
            <div className="skills-view-list">
              <List aria-label="Skills" size="sm">
                {count > 0 && (
                  <ListFoldSection id="in-chat" title="In this chat" count={count}>
                    {inChat.map(card)}
                    <ListRun
                      id="built-in"
                      items={builtIns}
                      standsOut={() => filtering}
                      says={(n) => `${n} built-in skills`}
                      mark={<Sparkle aria-hidden="true" />}
                    >
                      {card}
                    </ListRun>
                  </ListFoldSection>
                )}
                {leftOut > 0 && (
                  <ListFoldSection id="left-out" title="Left out" count={leftOut} defaultFolded={false}>
                    {writing === "one-line" && off.length > 0 ? (
                      <ListRun
                        id="superpowers"
                        items={off}
                        says={(n) => `superpowers: ${n} skills. ${state.line}`}
                        closing={{
                          says: state.withFix,
                          children:
                            state.command === undefined ? undefined : (
                              <Button size="sm" variant="ghost" onPress={() => copy(state.command!)}>
                                Copy command
                              </Button>
                            ),
                        }}
                      >
                        {card}
                      </ListRun>
                    ) : (
                      off.map(card)
                    )}
                    {hidden.map(card)}
                  </ListFoldSection>
                )}
              </List>
            </div>
          </div>
        </aside>
        <section className="proto-notes">
          <h1>Skills view: a plugin the chat does not have</h1>
          <p className="honest">
            Prototype for #4477. The list is drawn from the real parts on stand-in data. Left out
            starts open here so the rows can be judged; the app folds it at first. Try filtering on
            super, as the operator did.
          </p>
          <h2>What each source says in this state</h2>
          <dl className="proto-facts">
            {state.facts.map(([where, says]) => (
              <div key={where}>
                <dt>{where}</dt>
                <dd>{says}</dd>
              </div>
            ))}
          </dl>
          <p className="honest">Measured on Claude Code 2.1.316 with a stand-in plugin.</p>
          <h2>The chat&apos;s input</h2>
          <p className="proto-input" role="status">
            {typed ?? <span className="honest">Press Use on a card. It is typed, and not sent.</span>}
          </p>
          {did !== undefined && <p className="honest">{did}</p>}
          <h2>This way of writing</h2>
          <p>{WRITINGS.find((one) => one.id === writing)?.says}</p>
          <p className="honest">
            The header&apos;s last count moves with the plugin: 16 skills on, 3 skills off. In the
            index skill stays apart: those two rows keep Use, which asks the chat to load them.
          </p>
        </section>
      </div>
      <div className="proto-bar" role="group" aria-label="Prototype controls, not part of the design">
        <div className="proto-bar-line">
        <span className="proto-bar-label">State</span>
        <ButtonRow>
          {STATES.map((one) => (
            <Button
              key={one.id}
              size="sm"
              variant={one.id === stateId ? "primary" : "ghost"}
              aria-pressed={one.id === stateId}
              onPress={() => {
                setState(one.id);
                setTyped(undefined);
                setDid(undefined);
              }}
            >
              {one.name}
            </Button>
          ))}
        </ButtonRow>
        </div>
        {stateId !== "on" && (
        <div className="proto-bar-line">
        <span className="proto-bar-label">The off row</span>
        <ButtonRow>
          {WRITINGS.map((one) => (
            <Button
              key={one.id}
              size="sm"
              variant={one.id === writing ? "primary" : "ghost"}
              aria-pressed={one.id === writing}
              onPress={() => setWriting(one.id)}
            >
              {one.name}
            </Button>
          ))}
        </ButtonRow>
        </div>
        )}
        {look !== undefined && onLook !== undefined && (
          <Button
            size="sm"
            variant="ghost"
            onPress={() =>
              onLook({ ...look, appearance: look.appearance === "dark" ? "light" : "dark" })
            }
          >
            {look.appearance === "dark" ? "Light" : "Dark"}
          </Button>
        )}
      </div>
    </div>
  );
}
