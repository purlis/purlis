// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { setSkillStudioOn, useSkillStudioOn } from "../skillStudio";
import { Choice } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Previews** (SK-2, #2765): features built behind a flag until they are finished, which
 * the person may turn on on this machine to try. Each writes this machine's store, never a
 * project file.
 */
export function previewsGroup(): SettingsGroup {
  return {
    id: PREVIEWS,
    label: "Previews",
    help: "Features still being built, which you can turn on on this machine to try. They may change or move before they are finished.",
    settings: [skillStudio],
  };
}

/** The group's address. */
export const PREVIEWS = "you.previews";

const skillStudio: LiveSetting = {
  id: `${PREVIEWS}.skill-studio`,
  label: "Skill studio",
  help: "Lists every skill your chats get, and gives each chat the project's own skills. Open it from the palette: Open Skill studio.",
  writes: "this machine",
  useControl: function useSkillStudioPreview() {
    const on = useSkillStudioOn();
    const [refused, setRefused] = useState<string>();
    return {
      error: refused === undefined ? undefined : [refused],
      control: (ids) => (
        <Choice
          kind="switch"
          ids={ids}
          checked={on}
          onCheckedChange={(to) => void setSkillStudioOn(to).then(setRefused)}
        />
      ),
    };
  },
};
