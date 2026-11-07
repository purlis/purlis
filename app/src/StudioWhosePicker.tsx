// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { StudioClone } from "./bindings";
import { Button } from "./ui/button/Button";
import { Menu, MenuItem, MenuSection, MenuTrigger, MenuWords } from "./ui/menu/Menu";
import { FILTERED_FROM, Select } from "./ui/select/Select";
import { counted } from "./dispatches";
import { sameScope, scopeChat, scopeKey, startedIn, type StudioScope } from "./skillStudio";
import { placeTitle } from "./studioPlace";

/** What the menu's Where section says under the workspace's root. */
const NO_REPO_COUNTS = "No repo's skills count.";

/** What stands under the menu's rows: a clone with no repo skills is no place of its own. */
export const WHERE_FOOT =
  "A chat in a clone that holds no repo skills lists what a chat at the root lists.";

/** The key of the Where row of the workspace's root. */
const AT_ROOT = "\u0000root";

/** **The trigger's words**: whose chats, then where the chat starts (#4227). */
export function whoseAndWhere(scope: StudioScope): string {
  return `${scopeChat(scope)} · ${scope.where === undefined ? "at its root" : `in ${scope.where}`}`;
}

/**
 * **Whose chats the studio shows, and where the chat it previews starts** (#2767, #4227): the
 * studio header's scope picker. Whose chats is one of `whose`: at the project's root, in a
 * workspace or as a persona. Where a workspace's clones hold repo skills (`places`), it gains
 * Where it starts (variant A of #4227's prototype): at the workspace's root, or in each of those
 * clones, whose repo skills a chat started there counts too. Both are one menu then, Whose chats
 * over Where it starts; with no place to choose it is the select it always was.
 */
export function StudioWhosePicker({
  scope,
  whose,
  places,
  onScope,
}: {
  scope: StudioScope;
  /** Every scope it offers, the one shown among them. */
  whose: readonly StudioScope[];
  /** The clones of the shown workspace a chat can be previewed in. */
  places: readonly StudioClone[];
  onScope: (next: StudioScope) => void;
}) {
  const [typed, setTyped] = useState("");
  const pick = (next: StudioScope) => {
    if (!sameScope(next, scope)) onScope(next);
  };
  if (scope.at !== "workspace" || places.length === 0)
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
  const long = whose.length >= FILTERED_FROM;
  const shown = long
    ? whose.filter((one) => scopeChat(one).toLowerCase().includes(typed.trim().toLowerCase()))
    : whose;
  return (
    <MenuTrigger onOpenChange={(open) => open || setTyped("")}>
      <Button size="sm" data-testid="studio-whose">
        {whoseAndWhere(scope)}
      </Button>
      <Menu
        align="end"
        aria-label="Whose chats and where they start"
        footer={WHERE_FOOT}
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
        <MenuSection
          title="Where it starts"
          selectionMode="single"
          selectedKeys={[scope.where ?? AT_ROOT]}
          shouldCloseOnSelect
        >
          {[undefined, ...places].map((place) => {
            const clone = place?.clone;
            return (
              <MenuItem
                key={clone ?? AT_ROOT}
                id={clone ?? AT_ROOT}
                textValue={placeTitle(clone)}
                onAction={() => pick(startedIn(scope, clone))}
                data-testid={`studio-where-${clone ?? "root"}`}
              >
                <MenuWords
                  said={placeTitle(clone)}
                  under={
                    place === undefined
                      ? NO_REPO_COUNTS
                      : `Its ${counted(place.repo_skills, "repo skill counts", "repo skills count")} too.`
                  }
                />
              </MenuItem>
            );
          })}
        </MenuSection>
      </Menu>
    </MenuTrigger>
  );
}
