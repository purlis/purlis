# Amendment, 2026-11-04 (#4351): a part is styled through its classes

The operator's ruling of 2026-11-04. **A part still carries `data-scope` and `data-part`, and
beside them the classes `ds-<scope>` and `dp-<part>`. Every rule that matches a scope or a part
exactly is keyed on those classes once it is built.** The stylesheets are still written against
the attributes, and the manifest, the theme files and every query of a part are unchanged.

- **Why:** WebKit files a rule keyed on an attribute under the attribute's name alone, so on
  every restyle of the whole window each of the ~2,900 elements carrying `data-scope` was tested
  against each of the ~700 rules keyed on any scope. A rule keyed on a class is tried only on
  elements with that class. On the outcome bars (#5001, run 41709589411), re-keying the rules
  cut the style time of one Skill studio opening from 279 ms to 150 ms (medians of 20 openings
  each), towards the 300 ms bar of L10.
- **Where the classes come from:** nobody writes them by hand.
  - The window's JSX is compiled against `app/src/ui/jsx-runtime.ts` (`jsxImportSource`), which
    adds the classes of the attributes an element ends with (`ui/partClasses.ts`).
  - A part made with `createElement` takes them from `parted()`.
  - A library panel's outer box takes them from `resizablePanelRoot()`, because the library puts
    a panel's `className` on its inner box.
- **Where the rules are keyed:** the build's PostCSS step (`vite.config.ts`) rewrites each
  exact `[data-scope=…]` or `[data-part=…]` match where it stands. The page rewrites the
  engine's parts stylesheet the same way as it draws it (`drawParts`). A presence test
  (`[data-part]`), a prefix match, a case-blind match and a value that can't name a class stay
  keyed on the attribute.
- **Cascade order and weight:** a class and an attribute selector both weigh (0,1,0), and each
  selector is rewritten in place, so no rule changes its specificity, layer or order. The
  forced-colours sheets are rewritten like the rest.
- **How it is held:**
  - After every story in Chromium (`.storybook/parts.setup.ts`), each element's part classes
    must be exactly the ones its attributes draw, and no rule on the page may still key a part
    on its attribute.
  - `PartClasses.window.test.tsx` holds the window to the same.
- **What it costs:**
  - A part drawn outside React, such as a scenario spec drawing one by hand, must add its
    classes itself, as `pane-crumbs`, `pane-notices`, `pane-warnings` and `forced-colors` do.
    So must a part an element is cloned into (`cloneElement`), as `Tooltip` does through
    `parted()`.
  - A React Aria component given a part's attributes no longer carries its default
    `react-aria-*` class. Nothing styles those classes.
