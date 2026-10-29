// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The panel family's lint** (`eslint.ui-rules.js` finds this file, #3387):
 *
 * `purlis/panel-family` and `purlis/panel-section-family` (errors: these families have moved,
 * #2913): each family's old classes, and its `data-scope` written by hand, which only its wrapper
 * draws.
 *
 * `purlis/view-frame-family` (error: a paged view is the family's view frame, #3666):
 * `data-scope="view-frame"` or `"view-nav"` written by hand, which only `ViewFrame.tsx` draws.
 *
 * `purlis/panel-separator-family` (error: every resize handle is the family's, #3143):
 * `data-scope="panel-separator"` written by hand, and the panel library's own `Separator`,
 * by name or through `import * as`.
 */

import { moved } from "../familyLint.js";

const panelFamily = moved({
  description: "A region panel is the shared Panel family",
  scopes: ["panel"],
  classes: [],
  message:
    "a region panel: draw it as `<Panel>`, or give a library's panel `panelRoot(variant)`, from `ui/panel/Panel` (#2913).",
});

const panelSectionFamily = moved({
  description: "A panel's section and its title are the shared panel section family",
  scopes: ["panel-section"],
  classes: ["sidebar-title", "panel-head"],
  message:
    "a panel's section: draw it as `<PanelSection>`, `<PanelSectionHead>` and `<PanelSectionTitle>` from `ui/panel/Panel` (#2913).",
});

const separatorScope = moved({
  description: "A resize handle between two panels is the shared panel separator family",
  scopes: ["panel-separator"],
  classes: [],
  message: "a resize handle: draw it as `<Separator>` from `ui/panel/Separator` (#3143).",
});

const viewFrameFamily = moved({
  description: "A view tab laid out as pages is the panel family's view frame",
  scopes: ["view-frame", "view-nav"],
  classes: [],
  message:
    "a view tab laid out as pages: draw it as `<ViewFrame>`, its list's heading as `<ViewListHead>` and its item's header as `<ViewPageHeader>`, from `ui/panel/ViewFrame` (#3666).",
});

/** The panel library, whose own handle only the family's wrapper draws. */
const PANEL_LIBRARY = "react-resizable-panels";

/**
 * **The panel separator family** (#3143): its `data-scope` by hand, as every family that has
 * moved; and the panel library's own `Separator`, taken by name or through the whole module
 * (`import * as`), which `no-restricted-imports` cannot see.
 */
const panelSeparatorFamily = {
  meta: {
    ...separatorScope.meta,
    messages: {
      ...separatorScope.meta.messages,
      library:
        "the panel library's own handle is drawn only by the family: draw it as `<Separator>` from `ui/panel/Separator` (#3143).",
    },
  },
  create(context) {
    return {
      ...separatorScope.create(context),
      ImportDeclaration(node) {
        if (node.source.value !== PANEL_LIBRARY) return;
        for (const one of node.specifiers) {
          const whole = one.type === "ImportNamespaceSpecifier";
          const named = one.type === "ImportSpecifier" && one.imported.name === "Separator";
          if (whole || named) context.report({ node: one, messageId: "library" });
        }
      },
    };
  },
};

/** This folder's rules, by name: `purlis/<name>` in `eslint.config.js`, an error. */
export const rules = {
  "panel-family": panelFamily,
  "panel-section-family": panelSectionFamily,
  "panel-separator-family": panelSeparatorFamily,
  "view-frame-family": viewFrameFamily,
};
