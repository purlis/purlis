// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#2935, branch prototype/studio-whose-personas). Never merges to main.
 *
 * Question: how does the Skill studio's "Whose chats" picker list the project's personas, and
 * does it offer a persona in a workspace (a persona dispatched into one)?
 *   A  One menu, a Personas section   today's menu grows "As a persona"; picking one opens "In
 *                                     which workspace" in place of "Where it starts".
 *   B  Two pickers, Who and Where     "Who" (no persona, or each persona) beside "Where" (the
 *                                     root, each workspace, each clone): any pairing.
 *   C  One list, pairings named       a single filtered list: the root, workspaces, personas, then
 *                                     only the pairings that really happened ("steward in
 *                                     smart-ide · 41 chats").
 * Built from app/src/ui's families on stand-in data; no command is sent.
 */
import { useState } from "react";
import { ArrowLeft, ArrowRight } from "lucide-react";
import { Badge } from "../ui/badge/Badge";
import { Button, IconButton } from "../ui/button/Button";
import { Menu, MenuItem, MenuSection, MenuTrigger, MenuWords } from "../ui/menu/Menu";
import { Select } from "../ui/select/Select";
import {
  MANY_PERSONAS,
  PERSONAS,
  WORKSPACES,
  clonesOf,
  countOf,
  dispatchedInto,
  fromKey,
  key,
  said,
  skillsOf,
  type Persona,
  type Scope,
} from "./data";
import { ProtoBar, useParam, type Look } from "./ProtoBar";
import "./prototype.css";

const VARIANTS = [
  { key: "A", name: "One menu, a Personas section", says: "Today's menu grows a section; a persona's own Where is the workspaces it was dispatched into." },
  { key: "B", name: "Two pickers, Who and Where", says: "Who and Where are chosen apart, so any persona pairs with any workspace or clone." },
  { key: "C", name: "One list, pairings named", says: "One filtered list; a persona in a workspace is listed only where it was dispatched." },
] as const;

const MOMENTS = [
  { key: "library", name: "Opened from the Library" },
  { key: "persona-page", name: "Opened from steward's page" },
  { key: "many", name: "Twelve personas" },
] as const;

const START: Record<string, Scope> = {
  library: { workspace: "smart-ide" },
  "persona-page": { persona: "steward" },
  many: { workspace: "smart-ide" },
};

type PickerProps = { scope: Scope; personas: readonly Persona[]; onScope: (to: Scope) => void };

/** A: today's one menu, with "As a persona"; a persona's Where is where it was dispatched. */
function PickerA({ scope, personas, onScope }: PickerProps) {
  const [typed, setTyped] = useState("");
  const long = personas.length > 6;
  const match = (words: string) => words.toLowerCase().includes(typed.trim().toLowerCase());
  const whose = [
    { id: "who:root", said: "A chat at the project's root", to: {} as Scope },
    ...WORKSPACES.map((one) => ({ id: `w:${one.name}`, said: `A chat in ${one.name}`, to: { workspace: one.name } })),
  ].filter((one) => match(one.said));
  const shownPersonas = personas.filter((one) => match(`A chat as ${one.name}`));
  const picked = scope.persona ? `p:${scope.persona}` : scope.workspace ? `w:${scope.workspace}` : "who:root";
  const into = dispatchedInto(scope.persona, personas);
  return (
    <MenuTrigger onOpenChange={(open) => open || setTyped("")}>
      <Button size="sm">{said(scope)}</Button>
      <Menu
        align="end"
        aria-label="Whose chats and where they start"
        {...(long ? { filter: { typed, onTyped: setTyped, takesKeyboard: true, none: "No workspace or persona matches" } } : {})}
      >
        <MenuSection title="Whose chats" selectionMode="single" selectedKeys={[picked]} shouldCloseOnSelect>
          {whose.map((one) => (
            <MenuItem key={one.id} id={one.id} textValue={one.said} onAction={() => onScope(one.to)}>
              <MenuWords said={one.said} />
            </MenuItem>
          ))}
        </MenuSection>
        <MenuSection title="As a persona" selectionMode="single" selectedKeys={[picked]} shouldCloseOnSelect>
          {shownPersonas.map((one) => (
            <MenuItem
              key={one.name}
              id={`p:${one.name}`}
              textValue={`A chat as ${one.name}`}
              onAction={() => onScope({ persona: one.name })}
            >
              <MenuWords said={`A chat as ${one.name}`} under={one.role || undefined} />
            </MenuItem>
          ))}
        </MenuSection>
        {scope.persona !== undefined ? (
          <MenuSection
            title="In which workspace"
            selectionMode="single"
            selectedKeys={[`into:${scope.workspace ?? "root"}`]}
            shouldCloseOnSelect
          >
            {[
              <MenuItem key="into:root" id="into:root" textValue="At the project's root" onAction={() => onScope({ persona: scope.persona })}>
                <MenuWords said="At the project's root" />
              </MenuItem>,
              ...into.map(([workspace, chats]) => (
                <MenuItem
                  key={workspace}
                  id={`into:${workspace}`}
                  textValue={`Dispatched into ${workspace}`}
                  onAction={() => onScope({ persona: scope.persona, workspace })}
                >
                  <MenuWords said={`Dispatched into ${workspace}`} under={`${chats} chats ran there`} />
                </MenuItem>
              )),
            ]}
          </MenuSection>
        ) : scope.workspace !== undefined && clonesOf(scope.workspace).length > 0 ? (
          <MenuSection title="Where it starts" selectionMode="single" selectedKeys={[`where:${scope.clone ?? "root"}`]} shouldCloseOnSelect>
            {[
              <MenuItem key="where:root" id="where:root" textValue="At its root" onAction={() => onScope({ workspace: scope.workspace })}>
                <MenuWords said="At its root" />
              </MenuItem>,
              ...clonesOf(scope.workspace).map((one) => (
                <MenuItem
                  key={one.clone}
                  id={`where:${one.clone}`}
                  textValue={`In ${one.clone}`}
                  tooltip={`Its ${one.repo} repo skills count too.`}
                  onAction={() => onScope({ workspace: scope.workspace, clone: one.clone })}
                >
                  <MenuWords said={`In ${one.clone}`} />
                </MenuItem>
              )),
            ]}
          </MenuSection>
        ) : null}
      </Menu>
    </MenuTrigger>
  );
}

/** B: Who beside Where. */
function PickerB({ scope, personas, onScope }: PickerProps) {
  const where = [
    { value: "", label: "The project's root", group: "The project" },
    ...WORKSPACES.flatMap((one) => [
      { value: one.name, label: one.name, group: "Workspaces" },
      ...one.clones.map((clone) => ({ value: `${one.name}|${clone.clone}`, label: `${one.name} › ${clone.clone}`, group: "Workspaces" })),
    ]),
  ];
  const whereNow = scope.workspace ? (scope.clone ? `${scope.workspace}|${scope.clone}` : scope.workspace) : "";
  return (
    <div className="proto-inline">
      <Select
        size="sm"
        aria-label="Who"
        value={scope.persona ?? ""}
        filtered={personas.length + 1 >= 10}
        options={[
          { value: "", label: "Any chat, no persona", group: "No persona" },
          ...personas.map((one) => ({ value: one.name, label: `As ${one.name}`, group: "Personas" })),
        ]}
        onValueChange={(to) => onScope({ ...scope, persona: to || undefined })}
      />
      <Select
        size="sm"
        aria-label="Where"
        value={whereNow}
        options={where}
        onValueChange={(to) => {
          const [workspace, clone] = to.split("|");
          onScope({ persona: scope.persona, workspace: workspace || undefined, clone: clone || undefined });
        }}
      />
    </div>
  );
}

/** C: one list; a persona in a workspace only where it was dispatched. */
function PickerC({ scope, personas, onScope }: PickerProps) {
  const options = [
    { value: key({}), label: "A chat at the project's root", group: "The project" },
    ...WORKSPACES.flatMap((one) => [
      { value: key({ workspace: one.name }), label: `A chat in ${one.name}`, group: "Workspaces" },
      ...one.clones.map((clone) => ({
        value: key({ workspace: one.name, clone: clone.clone }),
        label: `A chat in ${one.name} › ${clone.clone}`,
        group: "Workspaces",
        tooltip: `Its ${clone.repo} repo skills count too.`,
      })),
    ]),
    ...personas.map((one) => ({ value: key({ persona: one.name }), label: `A chat as ${one.name}`, group: "Personas" })),
    ...personas.flatMap((one) =>
      one.into.map(([workspace, chats]) => ({
        value: key({ persona: one.name, workspace }),
        label: `${one.name} in ${workspace} · ${chats} chats`,
        group: "Where a persona was dispatched",
      })),
    ),
  ];
  return (
    <Select
      size="sm"
      aria-label="Whose chats"
      value={key(scope)}
      filtered
      options={options}
      onValueChange={(to) => onScope(fromKey(to))}
    />
  );
}

export function PrototypeWhose({ look, onLook }: { look?: Look; onLook?: (to: Look) => void }) {
  const [variant, setVariant] = useParam("variant", VARIANTS.map((one) => one.key));
  const [moment, setMoment] = useParam("moment", MOMENTS.map((one) => one.key));
  const [scope, setScope] = useState<Scope>(START[moment]!);
  const personas = moment === "many" ? MANY_PERSONAS : PERSONAS;
  const Picker = variant === "B" ? PickerB : variant === "C" ? PickerC : PickerA;
  const rows = skillsOf(scope);
  const count = countOf(scope);
  return (
    <div className="proto-page">
      <div className="proto-stage">
        <section className="proto-window" aria-label="The Skill studio, as its tab draws it">
          <div className="proto-head">
            <IconButton size="sm" aria-label="Back" icon={<ArrowLeft aria-hidden="true" />} />
            <IconButton size="sm" aria-label="Forward" icon={<ArrowRight aria-hidden="true" />} />
            <h2>Library</h2>
            <Picker scope={scope} personas={personas} onScope={setScope} />
          </div>
          <div className="proto-body">
            <nav className="proto-nav" aria-label="Pages">
              {["Library", "Needs you", "Drafts", "Sources", "Deleted"].map((page) => (
                <Button key={page} size="sm" variant={page === "Library" ? "soft-accent" : "ghost"}>
                  {page}
                </Button>
              ))}
            </nav>
            <div className="proto-main">
              <p>
                <strong>{said(scope)}</strong> gets {count} skills.
                {scope.persona && scope.workspace && (
                  <span className="honest"> As a dispatched chat sees them: the workspace&apos;s skills and the persona&apos;s, both.</span>
                )}
              </p>
              <ul className="proto-rows" aria-label="Skills this chat gets">
                {rows.slice(0, 14).map((one) => (
                  <li key={`${one.level}-${one.name}`}>
                    <code className="proto-grow">{one.name}</code>
                    <Badge variant={one.level === "Persona" ? "accent" : one.level === "Built in" ? "neutral" : "ok"}>{one.level}</Badge>
                  </li>
                ))}
              </ul>
              {rows.length > 14 && <p className="honest">and {count - 14} more</p>}
            </div>
          </div>
        </section>
        <section className="proto-notes">
          <h1>Whose chats: the project&apos;s personas (#2935)</h1>
          <p className="honest">
            Prototype on stand-in data, drawn from the real families. Open the picker in the
            studio&apos;s head and pick a persona, then a workspace. Nothing is sent.
          </p>
          <h2>Picked now</h2>
          <p className="proto-said" role="status">
            {JSON.stringify({ workspace: scope.workspace ?? null, persona: scope.persona ?? null, clone: scope.clone ?? null })}
          </p>
          <h2>What this variant decides</h2>
          <ul>
            {variant === "A" && (
              <>
                <li>The menu people know, one section longer. Personas sit under their own title, with their role under the name.</li>
                <li>A persona in a workspace is offered only where it was dispatched, under &quot;In which workspace&quot;, which takes the place of Where it starts.</li>
                <li>A persona and a clone together is not offered.</li>
              </>
            )}
            {variant === "B" && (
              <>
                <li>Two short selects. Who is &quot;Any chat&quot; or a persona; Where is the root, a workspace or a clone.</li>
                <li>Every pairing can be picked, including ones that never happened, such as qa in billing › nothing.</li>
                <li>Costs a second control in the studio&apos;s head, which is already crowded when the window is narrow.</li>
              </>
            )}
            {variant === "C" && (
              <>
                <li>One select with a filter: each line is a whole answer, and the pairings are the ones dispatch history knows.</li>
                <li>Long once there are many personas and workspaces; the filter carries it.</li>
                <li>A pairing that never happened cannot be picked, so a person cannot look at a chat that no one starts.</li>
              </>
            )}
          </ul>
        </section>
      </div>
      <ProtoBar
        variants={VARIANTS}
        variant={variant}
        onVariant={setVariant}
        moments={MOMENTS}
        moment={moment}
        onMoment={(to) => {
          setMoment(to);
          setScope(START[to]!);
        }}
        look={look}
        onLook={onLook}
      />
    </div>
  );
}
