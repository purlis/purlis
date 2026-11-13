// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { StudioClone, StudioPersona } from "./bindings";
import { Button } from "./ui/button/Button";
import { Menu, MenuItem, MenuSection, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { FILTERED_FROM, Select } from "./ui/select/Select";
import { counted } from "./dispatches";
import {
  asPersona,
  asPersonaIn,
  sameScope,
  scopeChat,
  scopeKey,
  startedIn,
  type StudioScope,
} from "./skillStudio";
import { placeTitle } from "./studioPlace";

/** **The note of the Where row of the workspace's root**, its tooltip (the menus' rule 2): a
 *  clone with no repo skills is no place of its own. */
export const ROOT_NOTE =
  "No repo's skills count. A chat in a clone that holds no repo skills lists the same.";

/** The key of the Where row of the workspace's root: no clone's name is it. */
const WHERE_ROOT_KEY = "\u0000root";

/** The key of an In which workspace row: no scope's key or clone's name starts with it. */
const INTO_ROW = "\u0000into:";

/** The In which workspace row of the project's root. */
const AT_THE_PROJECTS_ROOT = "At the project's root";

/** **The trigger's words**: whose chats, then where the chat starts (#4227). */
export function whoseAndWhere(scope: StudioScope): string {
  return `${scopeChat(scope)} · ${scope.where === undefined ? "at its root" : `in ${scope.where}`}`;
}

/**
 * **Whose chats the studio shows, and where the chat it previews starts** (#2767, #4227): the
 * studio header's scope picker. Whose chats is one of `whose`: at the project's root, in a
 * workspace or as a persona. Where a workspace's clones hold repo skills (`places`), it gains
 * Where it starts (variant A of #4227's prototype): at the workspace's root, or in each of those
 * clones, whose repo skills a chat started there counts too. Where the project has personas
 * (#2935, variant A of its prototype) it gains As a persona, each with its role under its name;
 * with a persona picked, In which workspace takes the place of Where it starts: the project's
 * root, or each workspace the persona was dispatched into, as a chat dispatched there sees its
 * skills. All are one menu then; with neither a place nor a persona to choose it is the select
 * it always was.
 */
export function StudioWhosePicker({
  scope,
  whose,
  personas,
  places,
  onScope,
}: {
  scope: StudioScope;
  /** Every scope on no persona it offers: the project's root and its workspaces. */
  whose: readonly StudioScope[];
  /** The project's personas, each with where it was dispatched (#2935). */
  personas: readonly StudioPersona[];
  /** The clones of the shown workspace a chat can be previewed in. */
  places: readonly StudioClone[];
  onScope: (next: StudioScope) => void;
}) {
  const [typed, setTyped] = useState("");
  const pick = (next: StudioScope) => {
    if (!sameScope(next, scope)) onScope(next);
  };
  // The persona shown is always among them, while its skills are still being read too.
  const shownPersona =
    scope.at === "persona"
      ? (personas.find((one) => one.name === scope.name) ?? {
          name: scope.name,
          role: null,
          dispatched_into: [],
        })
      : undefined;
  const notListed =
    shownPersona === undefined || personas.includes(shownPersona) ? [] : [shownPersona];
  const everyPersona = [...personas, ...notListed];
  const inAClone = scope.at === "workspace" && places.length > 0;
  if (!inAClone && everyPersona.length === 0)
    return (
      <Select
        size="sm"
        aria-label="Whose chats"
        value={scopeKey(scope)}
        options={whose.map((one) => ({ value: scopeKey(one), label: scopeChat(one) }))}
        onValueChange={(to) => {
          const next = whose.find((one) => scopeKey(one) === to);
          if (next !== undefined) pick(next);
        }}
      />
    );
  const long = whose.length + everyPersona.length >= FILTERED_FROM;
  const matches = (said: string) =>
    !long || said.toLowerCase().includes(typed.trim().toLowerCase());
  const shown = whose.filter((one) => matches(scopeChat(one)));
  const shownPersonas = everyPersona.filter((one) => matches(scopeChat(asPersona(one.name))));
  // Where the persona shown was dispatched, and the one it is shown in, while that is read too.
  const into = shownPersona?.dispatched_into ?? [];
  const intoShown =
    scope.into === undefined || into.some((one) => one.workspace === scope.into)
      ? into
      : [...into, { workspace: scope.into, chats: 0 }];
  return (
    <MenuTrigger onOpenChange={(open) => open || setTyped("")}>
      <Button size="sm" data-testid="studio-whose">
        {inAClone ? whoseAndWhere(scope) : scopeChat(scope)}
      </Button>
      <Menu
        align="end"
        aria-label="Whose chats and where they start"
        {...(long
          ? {
              filter: {
                typed,
                onTyped: setTyped,
                takesKeyboard: true,
                none: "No workspace or persona matches",
              },
            }
          : {})}
      >
        <MenuSection
          title="Whose chats"
          selectionMode="single"
          selectedKeys={[scopeKey(scope)]}
          shouldCloseOnSelect
        >
          {shown.map((one) => (
            <MenuItem
              key={scopeKey(one)}
              id={scopeKey(one)}
              textValue={scopeChat(one)}
              // The workspace shown, picked again, keeps where its chat starts.
              onAction={() => scopeKey(one) !== scopeKey(scope) && pick(one)}
            >
              <MenuWords said={scopeChat(one)} />
            </MenuItem>
          ))}
        </MenuSection>
        {shownPersonas.length > 0 ? (
          <MenuSection
            title="As a persona"
            selectionMode="single"
            selectedKeys={shownPersona === undefined ? [] : [scopeKey(asPersona(scope.name))]}
            shouldCloseOnSelect
          >
            {shownPersonas.map((one) => {
              const as = asPersona(one.name);
              return (
                <MenuItem
                  key={scopeKey(as)}
                  id={scopeKey(as)}
                  textValue={scopeChat(as)}
                  // The persona shown, picked again, keeps the workspace it is shown in.
                  onAction={() => shownPersona?.name !== one.name && pick(as)}
                  data-testid={`studio-as-${one.name}`}
                >
                  <MenuWords said={scopeChat(as)} under={one.role ?? undefined} />
                </MenuItem>
              );
            })}
          </MenuSection>
        ) : null}
        {shownPersona !== undefined ? (
          <MenuSection
            title="In which workspace"
            selectionMode="single"
            selectedKeys={[`${INTO_ROW}${scope.into ?? ""}`]}
            shouldCloseOnSelect
          >
            {[undefined, ...intoShown].map((one) => {
              const id = `${INTO_ROW}${one?.workspace ?? ""}`;
              const said =
                one === undefined ? AT_THE_PROJECTS_ROOT : `Dispatched into ${one.workspace}`;
              return (
                <MenuItem
                  key={id}
                  id={id}
                  textValue={said}
                  onAction={() => pick(asPersonaIn(scope.name, one?.workspace))}
                  data-testid={`studio-into-${one?.workspace ?? "root"}`}
                >
                  <MenuWords
                    said={said}
                    under={
                      one === undefined || one.chats === 0
                        ? undefined
                        : counted(one.chats, "chat ran there", "chats ran there")
                    }
                  />
                </MenuItem>
              );
            })}
          </MenuSection>
        ) : null}
        {inAClone ? (
          <MenuSection
            title="Where it starts"
            selectionMode="single"
            selectedKeys={[scope.where ?? WHERE_ROOT_KEY]}
            shouldCloseOnSelect
          >
            {[undefined, ...places].map((place) => {
              const clone = place?.clone;
              return (
                <MenuItem
                  key={clone ?? WHERE_ROOT_KEY}
                  id={clone ?? WHERE_ROOT_KEY}
                  textValue={placeTitle(clone)}
                  tooltip={
                    place === undefined
                      ? ROOT_NOTE
                      : `Its ${counted(place.repo_skills, "repo skill counts", "repo skills count")} too.`
                  }
                  onAction={() => pick(startedIn(scope, clone))}
                  data-testid={`studio-where-${clone ?? "root"}`}
                >
                  <MenuWords said={placeTitle(clone)} />
                </MenuItem>
              );
            })}
          </MenuSection>
        ) : null}
      </Menu>
    </MenuTrigger>
  );
}
