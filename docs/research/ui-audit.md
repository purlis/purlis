# purlis app UI audit: inventory for a UI-system program

**Source.** purlis/purlis `main` @ `47dab253b050afaf53eef11430cff4e54df29c2a` (committed 2026-10-20T12:33Z). It was read from source only: nothing was built, run or installed. `gh repo clone` failed inside this session's sandbox, which refuses writes to `.git/hooks` and `.git/config`, and the tarball download stalled. So the 535 non-test source files under `app/src`, `crates/purlis-core/src/theme` and `app/src-tauri/plugin` were fetched blob by blob at that exact commit through `gh api`. `app/src-tauri/src`, `docs/` and the tests were **not** fetched; the places where that limits a finding are flagged.

**Conventions.** Paths are repo-relative. A bare `App.css` means `app/src/App.css`, and a bare `Foo.tsx` means `app/src/Foo.tsx`. Counts are static greps over non-test `.tsx`/`.css`; where a count method matters it is named. Pixel heights are computed at the default 14px root (`app/src/theme/geometry.css:26`): `--size-control` = 28px and `--size-chrome` = 21px. View panes render at 12px (`App.css:6248`), so a padding-sized button is smaller there. Items marked **check on screen** are inferred from CSS and not seen rendered.

**How the CSS is layered** (`app/src/styles.css:75-86`). The layer order is `theme, base, vendor, charter (App.css), components (primitives/*.css), utilities, forced`. Because `components` beats `charter`, a primitive class such as `.ui-button` wins over any App.css rule whatever its specificity. That is why App.css restyles of buttons work only on buttons that do *not* carry `ui-button`. It is also the root of most "same class name, two looks" cases below.

**Headline numbers.**
- `App.css` has 8,683 lines. The primitives are `controls.css` (654 lines), `collections.css` (370), `overlays.css` (102) and `text.css` (73).
- 436 real `<button>`s:
  - 266 use `.ui-button`;
  - 80 have no class;
  - about 90 have ad-hoc classes.
- Colour is fully tokenised: App.css has **0** hex, rgb or hsl literals. Font sizes are fully tokenised too, with 0 literal px apart from the root.
- What is not tokenised:
  - shape: 8 non-token radii, including `border-radius: 0` on tabs and rows;
  - 21 literal `outline: 2px` focus rings;
  - 85 literal `1px` borders, with no border-width token;
  - 155 literal widths and heights;
  - about 36 literal opacities used as text ranks;
  - strip and title-bar heights.
- Overlays: 50 dialogs, 1 shared dialog surface, and 48 of those dialogs share one placement rule with **no sticky footer**.

Contents:
- **1. Components and patterns**
  - 1a: controls
  - 1b: strips, panels, notices, title bar and status bar
  - 1c: settings components, lists and cards
- **2. Every dialog**
- **3. Settings forms**
- **4. Typography**
- **5. Button sizing**
- **6. Theme reach**
- **7. Extension seams**
- **8. Top 15 inconsistencies**

---

# 1a. Controls: buttons, icon buttons, inputs, selects, switches, checkboxes, radios, segmented controls, menus, badges

### A.0 How controls are styled: four tiers

The cascade order is set in `app/src/styles.css:75`: `theme, base, vendor, charter(App.css), components(primitives/*.css), utilities, forced`. That gives a control one of four looks:

| Tier | Where | What draws it | Buttons that use it |
|---|---|---|---|
| 1. Primitive class | `app/src/primitives/controls.css` (components layer, which beats App.css) | `.ui-button*`, `.ui-input`, `.ui-select`, `.ui-checkbox`, `.ui-radio`, `.ui-switch`, `.ui-menu*`. Tokens only (the header comment at `controls.css:13` says "Only tokens"). | **266** of the 436 real `<button>` tags |
| 2. Unclassed baseline | `app/src/base.css:31-60` (`:where(...)`, zero specificity) | Token look, 28px from padding | **80** `<button>` tags have no `className` at all. Many of them are then restyled by a container rule in App.css (tier 3), for example `.answer button` and `.ui-setting-actions button`. |
| 3. Ad-hoc view class / container rule | `app/src/App.css` (8,683 lines, charter layer) | Bespoke padding, border, radius and font per view | **~90** `<button>` tags with a non-`ui-button` class, plus the unclassed ones that sit inside a styled container |
| 4. Tailwind utilities | `className="..."` strings | Utilities that map to tokens through `@theme` (`styles.css:246`, `--spacing: var(--space-2)`) | 119 `className`s carry utility classes. One control (the Chips choice, `settings/components.tsx:777`) is built entirely from them. |

Counted with a Python scan for `<button(?=[\s>])` that skips comment lines, then reads `className`. Running `grep -o '<button'` over everything gives 472, but that includes comment mentions.

### A.1 Buttons

**Shared primitive: `.ui-button`** (`primitives/controls.css:37-160`)
- Base rule `:37-55`. It is 28px tall (`block-size: var(--size-control)`), with `--radius-md`, a `--border-strong` 1px edge, `--control-base` ground, `--font-size-base` text, and `cursor: default`.
- Variants: `-primary` `:61`, `-ghost` `:70`, `-danger` `:81`, `-grants` `:94`, `aria-pressed` `:102`, and `-sm` `:111`, which is 21px tall with `--font-size-sm`. Disabled buttons drop to opacity 0.5 (`:118`). The focus ring uses `--focus-ring-width`/`--focus-ring-offset` (`:128`).
- `.ui-button-row` (`:139-160`) is a wrapping row. Inside it a button has `block-size: auto; min-block-size: control/chrome; white-space: normal`, so **a long label wraps and that button grows taller than its siblings**.
- Hard-coded values: only the 1px border and opacity .5. The file is clean.

Call sites, by class token and variant combination:

| Combination | Count | Example |
|---|---|---|
| `ui-button` (secondary) | 118 | `NewVault.tsx:143` |
| `ui-button-sm` | 52 | `Explorer.tsx:595` |
| `ui-button-primary` | 36 | `NewVault.tsx:135` |
| `ui-button-ghost ui-button-sm` | 24 | `StatusLine.tsx:361`, `Inbox.tsx:862` |
| `ui-button-danger` | 19 | `QuitWarning.tsx:118` |
| `ui-button-grants ui-button-sm` | 4 | `SandboxBlockNotice.tsx:978` |
| `ui-button-grants` | 3 | `StartChat.tsx:571` |
| `ui-button-primary ui-button-sm` | 2 | `BrowserView.tsx:540` |
| `ui-button-ghost` (full size) | 2 | `settings/SettingsTab.tsx:1182`, `ChatBrowsersShown.tsx:90` |

The class tokens appear 271 times in 81 files, and `ui-button-row` 16 times in 10 files. 40 distinct view classes ride on `.ui-button` for placement only: `ends-it` ×17, `panel-view` ×12, `inbox-go` ×9, `finished-clear` ×5, `ui-setting-reset` ×5, and others. Because the components layer wins, these classes **cannot** resize a `.ui-button`. That is why the same class name looks different with and without `ui-button` (see A.1b).

**Ad-hoc button looks in App.css.** Each is its own copy of "a button", with its own numbers:

| Rule | Line | Height at 14px | Radius | Border | Font | Call sites |
|---|---|---|---|---|---|---|
| `.answer button` (dialog `AnswerBar`) | App.css:4443 | ~30px (padding `--space-2`) | md | `--border-subtle` | inherit | unclassed buttons in `AnswerBar` (e.g. `ChangeActions.tsx:67`) |
| `.ui-setting-actions button` (`SettingActions`) | App.css:8406 | ~30px (`--space-2`/`--space-4`) | md | subtle; disabled .45; `cursor: pointer`; outline `2px` literal | inherit | ~30 unclassed buttons in 21 `SettingActions` rows (list in A.5) |
| `.ui-setting-reset` (raw) | App.css:8355 | ~26px (`--space-1`) | md | subtle; pointer | inherit | 13 raw (`settings/Collection.tsx:145,178,189,201,212,393,482`, `SettingsTab.tsx:530,757,1035`, `thisMachine.tsx:225`, `GrantedList.tsx:446`, `accounts.tsx:184`) |
| `.panel-view` | App.css:6990 | ~17px (padding 0, `--font-size-xs`) | **sm** | subtle; `--text-secondary` | xs | 10 raw (`AnswerQuestion.tsx:159,215,221`, `PanelSection.tsx:99`, `Views.tsx:468,763,896,1001`, `RegionFrame.tsx:709`, `ActivityTab.tsx:534`), plus 12 on `.ui-button-sm` (`SavingView.tsx:309…`) |
| `.title-about` | App.css:5850 | ~17px (`--space-0` padding, xs inherited from `.title-bar`) | md | subtle; `--text-secondary` | xs | 1 (`About.tsx:98`) |
| `.status-update` | App.css:5402 | ~17px | md | subtle; `--text-primary`/muted/accent by state | xs | 1 (`Updates.tsx:315`) |
| `.title-kill-switch` | App.css:5869 | ~17px | md | subtle | xs | 1 (`KillSwitch.tsx:92`) |
| `.needs-you-button` | App.css:4627 | ~15px, **no border**, no ground | md | none | xs | 1 (`NeedsYou.tsx:232`) |
| `.task-end-confirm button` | App.css:1604 | 21px (`--size-chrome`) | md | subtle; `--surface-raised` ground | xs | `TaskEnd.tsx:310,313` |
| `.bar button` | App.css:995 | `--space-1` padding | md | subtle | inherit | strip bars |
| `.welcome button, .onboarding-on button` | App.css:4957 | ~30px (`--space-2`/`--space-5`) | **lg** | subtle; transparent | inherit | `Welcome.tsx:522,551`, `Onboarding.tsx:189,296,305` |
| `.tabs .tab-tasks button` | App.css:1994 | 21px | **full** (pill) | none | sm | `TabChip.tsx:501,524` |
| `.projects button` | App.css:732 | `--space-1` | md | none | inherit | project strip |
| `.project .closer` / `.workspace .gear` | App.css:510 / :547 | padding 0 | **0** | none | inherit | `StripMarks.tsx:361,395` |
| `.pane-find button` | App.css:1374 | `--space-1` | sm | none | inherit | `FindBar.tsx:158,161,164` (icon `size={14}`) |
| `.pane-doing button` | App.css:2514 | `--space-1` | sm | none | inherit | pane header |
| `button.show-more` | App.css:1082 | `--space-1` | md | none | inherit | `StripMarks.tsx:254`, 2 sites |
| `button.bare` | App.css:1032 | — | md | none | — | **0** tsx call sites found (likely dead) |
| `.recents li button` | App.css:5156 | — | **0** | none | — | Welcome recents |

Hard-coded values across App.css:
- **21** focus outlines are written `outline: 2px solid` instead of `var(--focus-ring-width)`, against 6 that use the token. Examples: App.css:157, 501, 538, 864, 5905, 8426. Four of them also hard-code `outline-offset: 2px` (4465, 7968, 8124, 8427). `.ui-levels .ui-level:focus-visible` hard-codes `-2px` (App.css:7845).
- **8** `border-radius` values are not tokens: `0` at 441 (`.tabs [role="tab"]`), 517 (`.project .closer`), 554 (`.workspace .gear`), 4902, 5162, and `50%` at 4485 and 7468.
- 124 `rem` literals sit outside comments. Icon sizes are written five ways: `0.75rem` (2048, 2554), `0.8rem` (1844, 2117, 2123, 2211), `0.85rem` (2268), `1rem` (316), `1.25rem` (2773), and `1em` (7×). There is no icon-size token. `.title-bar { min-height: 2.1rem }` (App.css:5802) is also a literal.
- No hex or rgb colours appear in App.css rules. Colour tokenisation is complete.

**Icon buttons.** There is no shared icon-button. Seven kinds exist:
- `ui-button-ghost ui-button-sm` holding only an icon: `Cockpit.tsx:138,158`, `SavingView.tsx:820,833`, `SearchTab.tsx:228`, `VaultTab.tsx:958`, `settings/profiles.tsx:222`.
- `.pane-find button` (radius sm, `--text-muted`).
- `.pane-doing button`.
- `.project .closer` and `.workspace .gear` (radius 0).
- The tab `×` (`.tab button`, App.css:832, which only sets `nowrap`).
- `.tabs .tab-tasks button` (pill).
- `.needs-you-mark.rolled-up` (App.css:3669, 3934 — the same rule written twice).

Explicit `size={n}` props on lucide icons are 14 ×3, 7 ×2 and 5 ×2. Every other icon takes its size from the `.lucide` rule (App.css:173) or from a per-view rem literal.

#### A.1b The same name, two looks

`ui-setting-reset` alone (13 sites) is ~26px with a subtle border. `ui-setting-reset ui-button` (`settings/components.tsx:379,395`, `profiles.tsx:222`, `ForgeTokenSignIn.tsx:186`) is 28px with a strong border. `ui-setting-reset ui-button ui-button-ghost` (`SettingsTab.tsx:1182`, "Move to…") is 28px with no border. **All three appear on the Settings tab.** `panel-view` alone is 17px/xs/radius-sm, while `panel-view ui-button-sm` is 21px/sm/radius-md. `ends-it` alone draws a danger edge under `.answer` and `.ui-setting-actions` (App.css:4469, 8431) and the raised xs chip in `.task-end-confirm`. On `ui-button-danger` it draws danger text on the control ground with a strong border.

### A.2 Inputs and textareas

- **`.ui-input`** (`controls.css:167-260`): 28px, `--surface-sunken`, `--border-strong`, `--radius-md`. Textarea variant `:183`, list field `:193`, leading-icon holder `.ui-input-lead` `:244`, which reads the view props `--ui-input-lead-*`. It has 22 occurrences in 15 files and 8 `ui-input-lead`.
- **`.ui-field`** (App.css:8445): a second field look, with padding `--space-2`/`--space-3`, `--border-subtle`, `--radius-md` and no fixed height. It is mostly paired as `ui-field ui-input`, where `ui-input` wins (4 sites). It stands alone on **`AnswerQuestion.tsx:181`** (the panel answer textarea), which therefore looks different from the only other textarea (`settings/components.tsx:650`, `ui-input ui-list-field`).
- Other inputs:
  - `.tab-rename` (`TabRename.tsx:111`, App.css:850): border-subtle, `--font-size-md`. It is the only `md`-size input.
  - `input type=range` (`settings/components.tsx:569`, `.ui-field-range`).
  - `input type=color` unclassed (`SettingsTab.tsx:1411`).
  - Native `input type=checkbox className="m-0"` in Chips (`settings/components.tsx:811`).
- Count: 25 `<input>` and 2 `<textarea>`. 19 use `ui-input`.

### A.3 Selects, checkboxes, radios, switches, segmented controls, menus

| Control | Defined | Call sites | Shared? | Notes |
|---|---|---|---|---|
| `Select` (Radix, `.ui-select` trigger + `.ui-menu.ui-select-list`) | `primitives/Select.tsx:51`; look `controls.css:200-229` | 7 (`DispatchesTab.tsx:247,260,270,283`, `BrowserView.tsx:527`, `SearchTab.tsx:193`, `settings/components.tsx:909`) | shared | `overlays/select.tsx` is only the portal/content wrapper. There is no native `<select>` in tsx. |
| Checkbox `.ui-checkbox` (Radix) | `controls.css:~310`; 12px = `size-control*3/7`, `--radius-sm` | 2 roots, both in `settings/components.tsx:853,882` (inside `Choice`) | shared via `Choice` | |
| Native checkbox chips | Tailwind string `CHIP` `settings/components.tsx:777-780` (`rounded-full px-1.5 text-sm border-border-subtle`) | 1 (Chips kind of `Choice`) | ad-hoc, Tailwind | a third chip/pill style |
| Radio `.ui-radio` + `.ui-radio-mark` | `controls.css:~315-330` | 1 root (`settings/components.tsx:926`) | shared via `Choice` | |
| Switch `.ui-switch` | `controls.css:~351-400`; track = `size-control/2`, `--radius-full` | 1 (`settings/components.tsx:834`) | shared via `Choice` | `<Choice>` itself has 73 call sites (switch / checks / radio / chips / select) |
| **Segmented `.ui-segmented`** | `controls.css:~405-460` (28px, `--border-strong`, `--radius-lg`, `-sm` = 21px) | **0** | dead | |
| **Segmented `.ui-levels` / `.ui-level`** (Radix RadioGroup) | App.css:7813-7856 (+ inbox override 2911-2934) | 2 (`settings/components.tsx:139` level switcher; `ScopeSwitch.tsx:43` → Chats `ChatsSection.tsx:891` and Inbox `Inbox.tsx:375`) | App.css copy | It is drawn by padding, not by `--size-control`: ~26px full and ~16px small (`--font-size-2xs`). It uses `--border-subtle` (the primitive uses strong), `cursor: pointer` (the primitive uses default), and a `-2px` literal focus offset. |
| Persona mark picker radios | `PersonaMarkPicker.tsx:106,132` (`.persona-mark-choice`, swatches) | 2 groups | ad-hoc | |
| Menu `.ui-menu` / `.ui-menu-item` | `controls.css:~484-650`; row ≥ 28px, `--radius-md`, `--ui-menu-indent` | `ui-menu` 12 in 8 files, `ui-menu-item` 21 in 8; `MenuWords` 7, `MenuTip` 4, `MenuKeys` 1 (`primitives/MenuRow.tsx`) | shared | |

### A.4 Badges, counts and chips

There is **no shared badge or count primitive**. Seven copies exist:

| Class | App.css | Radius | Ground | Text | Size | Used by |
|---|---|---|---|---|---|---|
| `.count-pill` | 590 | full | `--control-count` | secondary | xs | `CountPill.tsx` (2 call sites) |
| `.activity-count` | 2803 | full | `--control-count`; needs-you tone | **primary** | xs, `min-width: 1rem` literal, tight line-height | activity bar |
| `.piece-count` | 3463 | full | `--control-count` | — | — | explorer |
| `.status-count` | 5390 | full | `--control-count` | — | xs | status bar Notices |
| `.search-count` | 7732 | full | `--control-count` | secondary | xs, padding `--space-3` (others use `--space-2`) | search tab |
| `.task-count` | 3636 | **sm**, with a **border** | — | — | tight | chats section |
| `.ui-setting-badge` | 8273 | sm, accent border | `--accent-surface` | primary | xs | 8 sites in 7 files (settings origin badge) |
| `.tab-tasks-counts .count` | 2032 | — | — | — | 21px tall; svg `0.75rem` literal | `TabChip.tsx:524` |
| `.ui-level-count` | 2927 | — | — | muted, then secondary when checked | — | scope switch |
| `.show-more-needs` | — | — | — | — | — | `StripMarks.tsx:262` |

Chips: `.tabs .tab.chip` (App.css:4610, `--chip` width), `.browser-chip` (8674, a border-end separator rather than a pill), `.pane-chips` (8 rules), and the Tailwind `CHIP`.

---

# 1b. Top tab strip, side panels, notices, title bar, status bar

#### Top tab strips (projects / workspaces / chats)

| Piece | Defined | Call sites | Shared or ad-hoc | Tokens / literals |
|---|---|---|---|---|
| Tab `<button role="tab">` | **written out 3 times**: `App.tsx:2093` (projects), `WorkspaceStrip.tsx:118-119` (workspaces), `ChatStrip.tsx` (4 `role="tab"`) | 3 strips | **Ad-hoc markup**; the styling is shared through one selector list in `App.css:370-560` | radius 0 (`:441`), padding `space-0 space-4`, `font-size-md` (`:447`), selected = `layer.selected` (`:409-416`), hover `surface-hover`, focus ring literal `2px`/`-2px` (`:501-502`) |
| Strip tablist | `StripTablist.tsx:33` | 3 | Shared | – |
| Sortable cell | `sortable.tsx` (`SortableTab`) | 3 | Shared | – |
| Closer `×` / Gear | `StripMarks.tsx:361` / `:395` | 2 / 2 | Shared | radius 0 (`App.css:517`, `:554`), padding `space-3` / `space-2` (they differ), focus literal |
| `+` and strip controls | `StripMarks.tsx` (`Doer`) | 9 in 4 files | Shared | – |
| Count pill | `CountPill.tsx` | 2 | Shared | `radius-full`, `control-count`, `text-secondary`, xs (`App.css:590-610`) |
| Cell width caps | — | — | — | `max-width: 18rem` (workspaces, `:390`), `15rem` (chats, `:394`), none for projects. These are literals. |
| Strip heights | — | — | — | title bar `2.1rem` literal (`:5802`), workspaces `--size-control`, chats `--size-chrome`. Each strip has a different height source. |
| Task chip / tasks menu | `TabChip.tsx` (1 226 lines) | 1 per chat tab | Shared (`ui-menu`) | — |

#### Side panels

| Piece | Defined | Sites | Shared/ad-hoc | Notes |
|---|---|---|---|---|
| Activity bar (vertical tablist) | `ActivityBar.tsx:101-130` | 1 (`RegionFrame`) | Shared | tab height `2.5rem` literal, icon `1.25rem` literal, selected = **inset 2px accent edge** (`App.css:2793-2799`), which differs from the strips' background fill; focus literal `2px` (`:2782`) |
| Activity count | `ActivityBar.tsx:166` | per tab | own class | `text-primary`, **strong**, xs, `min-width: 1rem` (`App.css:2803-2819`) |
| Panel section heading | `PanelSection.tsx:58-66` | 2 (+7 `HeadingOffer`) | Shared | `.sidebar-title` **plus** `.ui-text-label`, which declare the same thing twice (`App.css:3530-3540`, `text.css`) |
| Panel list (search, bound, load-more) | `PanelList.tsx` | 4 in 3 files | Shared; uses `ui-row` ×4 | — |
| Explorer sections and tree | `Explorer.tsx` (2 199 lines) | — | `sidebar-title explorer-section-head ui-text-label` ×2; `ui-tree` ×13, `ui-row` ×8 | headings are its own markup, not `PanelSection` |
| Inbox | `Inbox.tsx` (1 420 lines) | — | **Ad-hoc**: 0 `ui-row`, its own `inbox-*` classes (~30). Headings `inbox-chain sidebar-title` ×4 (`App.css:2947-2957` repeats `.sidebar-title`'s size, weight and colour) | an ask card mixes full-size `ui-button` answers with `ui-button-ghost ui-button-sm` "Go to chat"/Ignore ×6 (DS-12, `App.css:2958-2960`) |
| Empty state | `EmptyState.tsx` | 53 in 24 files | Shared | — |
| Context menus | `Menus.tsx` (`Menued`) | 22 in 14 files | Shared | — |

#### Notices

- **Shared component:** `Notice.tsx:138` (and `NoticeList` `:381`, `NoticePaneRow` `:494`). There are **136 `<Notice>` call sites in 59 files**. All 26 `*Notice*.tsx` files go through it; none is ad-hoc (`SandboxBlockNotice` 12, `ChatNotices` 9, `ProjectDispatchNotice` 4 …). A type guard means no Notice compiles without a way out (`Notice.tsx:92-100`), and `guard.test` refuses a standing line built any other way.
- **Buttons in one Notice:**
  - fixes are `ui-button ui-button-sm`, secondary and bordered (`Notice.tsx:203-218`);
  - a link is the same (`:220-229`);
  - Copy command is the same (`:230-240`);
  - **Dismiss is `ui-button-sm ui-button-ghost`**, with no border (`:241-251`);
  - grant answers add `ui-button-grants` (`:208`).
  So "Update workspace layout" (`InboxAlerts.tsx:199`, a fix) is bordered and "Dismiss" beside it is borderless at the same height. **No Notice has a primary button.**
- **Surfaces:** a standalone Notice is `ui-inline-surface`, red on trouble. In a pane it is `notice-pane-box ui-inline-surface ui-inline-surface-waiting`, which is always edged amber, even for news (`Notice.tsx:262-265`). In the Inbox the list row is the surface. That gives one component three looks.
- **Notice-like text that is not a Notice:** `.honest` paragraphs (82 sites in 22 files, `opacity: 0.8`) and 101 `role="alert"` elements. Most are dialog body text and trouble lines. Another fork covers dialogs.

#### Title bar (`TitleBar.tsx:161-200`; `App.css:5796-5906`)

Its right end has six items, drawn in **four different button styles**:

| Item | Class | Look |
|---|---|---|
| Needs-you | `.needs-you-button` (`App.css:4627-4638`) | borderless, padding `space-0 space-2`, `text-primary`, count = `.needs-you-number` (strong, `min-width 1.3rem`) |
| Save indicator | `ui-button ui-button-ghost ui-button-sm` (`SavingView.tsx:34, 55, 68`) | ghost, `block-size: --size-chrome`, `font-size-sm` |
| Sandbox mark | `.title-bar-sandbox` | a mark, not a button |
| Kill switch | `.title-kill-switch` (`App.css:5868-5880`) | ad-hoc: bordered `border-subtle`, `radius-md`, padding `space-0 space-3`, `text-secondary`, `font: inherit` (xs), **no block-size** |
| About | `.title-about` (`App.css:5850-5862`, `About.tsx:98`) | the same ad-hoc copy, `text-secondary` |
| Update | `.status-update` (`App.css:5402-5431`, `Updates.tsx:317`) | the same ad-hoc copy, but **`text-primary`**, and `accent-base` text plus border when an update is offered or installed |

- About, Kill switch and Update are three copies of one ad-hoc rule instead of `ui-button`. Their height comes from content (xs text), while the save indicator beside them is 21px.
- The comment above `.title-about` says "the same chrome as the update item beside it — one row, one shape" (`App.css:5849`). The shape matches, but the colour and weight do not: About is secondary and Update is primary or accent.
- `.status-update` is named for the status bar but sits in the title bar.
- Focus rings for all three are a literal `2px` (`App.css:5902-5906`).

#### Status bar (`StatusLine.tsx:189-273`; `App.css:5286-5390`)

- `font-size-sm` (12px), where the title bar is `font-size-xs` (11px), and padding `space-2 space-4`.
- The cells are text with `.status-label` and need no buttons. The alerts button is `ui-button ui-button-ghost ui-button-sm` (`StatusLine.tsx:361, 379`), and the pin is the same (`Updates.tsx:548`). The project item is a tooltip trigger with a **token** focus ring (`App.css:5340-5343`), the only strip control that uses the tokens.
- Extension badges are `status-cell status-badge`, muted when stale (`App.css:5380-5383`).


---

# 1c. Settings components, settings scope tabs, lists and cards

### C1. The settings component set (`app/src/settings/components.tsx`)

| Piece | Defined | Call sites | Shared? | Geometry / colour | Hard-coded or drifting |
|---|---|---|---|---|---|
| `SettingsLayout` (the two-column page, level switcher, filter, group nav) | components.tsx:79; CSS App.css:7790-7958 | 1 (SettingsTab) | shared | grid `minmax(9rem,12rem) minmax(0,48rem)` App.css:7792; gaps `--space-5/--space-7` | the column widths are literal rem values, not tokens |
| Level switcher (`.ui-levels`/`.ui-level`), the "settings scope tabs" | components.tsx:139-167; App.css:7813-7855 | 2 (SettingsLayout and ScopeSwitch) | shared, but **a second segmented style** | `--radius-lg`, `--border-subtle`, `--accent-surface`; the height comes from padding `--space-1` (App.css:7821), not from `--size-control` | duplicates `.ui-segmented` in primitives/controls.css:405-460, which has **0 call sites** in tsx and differs: `--border-strong`, `block-size: --size-control`, `font-size: base`. The settings level switcher therefore doesn't match the documented segmented primitive. |
| Chats/Inbox scope switch (`ScopeSwitch.tsx`) | ScopeSwitch.tsx:43 (`ui-levels ui-levels-small`) | 8 refs in 3 files | shared (`.ui-levels`) | `.ui-levels-small` font `--font-size-2xs` (10px) App.css:7835, padding `--space-0 --space-3` | the 10px size is below the window's own 11px floor (geometry.css:39-40 says so) |
| Group nav buttons (`.ui-settings-nav button`) | components.tsx:196-209; App.css:7885-7919 | 1 | ad-hoc rule on bare `<button>` | `--radius-md`, `--list-selected`, inset edge | the selected edge is `inset 2px` (App.css:7908, 7913), while `.ui-row` uses `--focus-ring-width` (collections.css:254). Same look, two values. |
| `SettingGroup` | components.tsx:227 | 2 rendering sites; one group is drawn at a time | shared | h3 `--font-size-md` App.css:7971 | **no font-weight**, so the UA's bold 700 applies, not `--font-weight-strong` (600) |
| `SettingRow` | components.tsx:304 | **136 references in 40 files**: 7 in settings/*, the rest in dialog forms (StartChat 7, VaultSignIn 8, NewProject/NewWorkspace/NewPersona/AskPersona 4 each, …) | shared, two modes: on a settings page help goes into an info-mark tooltip (`QuietHints`, components.tsx:47, 371, 413); in a dialog help is drawn as a line under the field | row `padding-block: --space-5` with a hairline App.css:8202-8207; line `flex-wrap` gap `--space-2 --space-5` App.css:8209 | see the alignment notes in C2 |
| Row label `.ui-setting-label` | App.css:8236 | every row | shared | `--font-weight-medium` (the only use of medium in the app besides 2 others); size inherited | see C4: the label is smaller than its own control's text |
| `HintMark` (info mark and tooltip) | components.tsx:439 | every row with help, on settings pages | shared | `--text-muted`; tooltip `max-inline-size: calc(--space-8 * 10)` App.css:8257 | `z-index: 20` literal |
| `OriginBadge` (`.ui-setting-badge`) | components.tsx:469; App.css:8273 | 8 refs in 7 files | shared | `--accent-base` border, `--accent-surface`, `--radius-sm`, xs | height comes from content, not chrome size |
| Reset / Undo (`.ui-setting-reset ui-button`) | components.tsx:377-406 | per row, only when the value is set at this level | shared | uses `.ui-button`, so 2rem high with 14px text (controls.css:42, 49) | — |
| "Move to…" | settings/SettingsTab.tsx:1180-1191 | every movable row | shared | `ui-setting-reset ui-button ui-button-ghost` | **ghost**, while Reset beside it is **bordered**. Two styles in one row. |
| `.ui-setting-reset` used alone (no `ui-button`) | Collection.tsx:145, 178, 189, 201, 212, 393, 482; GrantedList, thisMachine, accounts, SettingsTab:530 | **13 sites in 5 files** (vs 4 + 1 ghost with `ui-button`) | same class, different box | App.css:8355-8364: `padding: --space-1 --space-4`, `--border-subtle`, `font: inherit` | about 23px high and 12px text in a view pane, against the 28px/14px Reset. The class name promises "a row's reset" and draws two different buttons. |
| `SettingActions` (a form's button row) | components.tsx:983; App.css:8380-8438 | **157 references in 42 files**; inside them 28 bare `<button>` in 12 files, 72 classed | shared wrapper; its buttons are partly ad-hoc | bare buttons get App.css:8406 (`padding --space-2 --space-4`, `--border-subtle`); `.ends-it` danger, 38 uses in 24 files | `opacity: 0.45` for disabled (App.css:8421) vs 0.5 on `.ui-segmented-item` (controls.css:446) and the base disabled tier. A third and a fourth button box sit beside `.ui-button`. |
| `Field` / `Choice` / list field | components.tsx:564, 828, 628 | `Field` 94 refs in 35 files | shared | `.ui-field` App.css:8445 (padding `--space-2 --space-3`, `--surface-sunken`, `--radius-md`) **and** `.ui-input` controls.css:167 (2rem, base) are both applied (components.tsx:504, 653) | two text-box definitions stacked; `textarea.ui-field` sm and mono (App.css:8459); `.ui-field-range` `max-width: 20rem` literal (App.css:8471) |
| `Collection` (ST-3: entries with Remove / Confirm / Rename / Open, and an Add form) | settings/Collection.tsx:136-250 | 15 refs in 6 files (hosts, my hosts, persona hosts, profiles, …) | shared | entry `margin-top: --space-5`, head `flex-wrap` gap `--space-3` (App.css:8164-8196); h4 `--font-size-sm`, no weight, so UA bold (App.css:8176) | buttons are bare `.ui-setting-reset` (the small box) |
| Granted list (`.granted-list`) | settings/GrantedList.tsx:160-209, 279; NetworkLists.tsx:178 | 4 refs in 3 files | ad-hoc `<ul>` | `li` flex-wrap baseline gap `--space-3` App.css:8030 | Remove is a **bare unclassed `<button>`** (base.css:34-64 tier), drawn inline after the sentence |
| Machine list (`.ui-machine-list`) | settings/thisMachine.tsx; App.css:8329-8353 | 3 refs in 3 files | ad-hoc | action pushed to the end with `margin-inline-start: auto` (App.css:8351) | the only list whose action is right-aligned |
| `.absent-list` (Repos › Not cloned here) | App.css:8389-8404 | 1 | ad-hoc | `justify-content: space-between` | another right-aligned variant |
| `.ui-table` / `.ui-table-fixed` / `.ui-table-room` | primitives/collections.css:20-176 | `<table>` used **5 times**: VaultTab:525, SavingView:523, DispatchesTab:295, settings/dispatch.tsx:377, settings/DispatchGrants.tsx:1371 | shared | cell padding vars, a container query that stacks rows under 29rem (collections.css:118) | `dispatch-table` column widths 18/40/17/25% literal (App.css:8052-8066); `vault-secrets` class reused on DispatchesTab for its look |

#### Row actions sit in three different places

In settings, an action on a list item is placed one of three ways:
1. **Inline after the name** (Collection head, App.css:8168; GrantedList `li`, App.css:8030). The button's x-position moves with the length of each name.
2. **At the row's end** (`.ui-machine-entry .ui-setting-reset { margin-inline-start: auto }`, App.css:8351; `.absent-list` `space-between`, App.css:8399).
3. **Inside the control flex, after the control** (`SettingRow`'s Reset, Undo and Move to…, components.tsx:374-408). Their x-position moves with the control's width.

---

# 2. Every dialog (and the overlays: menus, popovers, tooltips)

### B.1 The overlay layer: what is shared

**CSS layer order** (`app/src/styles.css:86`): `theme, base, vendor, charter, components, utilities, forced`.
- `App.css` is in `charter`.
- `primitives/controls.css` and `primitives/overlays.css` are in `components`, so a class there beats any `App.css` rule on the same property, whatever its specificity.

**Wrappers.** `app/src/overlays/*.tsx` are thin re-exports of Radix. Each part is wrapped in `onlyWhileShown`, so the overlay is drawn only while its project is in front. There is no React look layer (ADR 0037).

| Wrapper | File | Content call sites |
|---|---|---|
| Dialog | `overlays/dialog.tsx:22-26` | 29 `<Dialog.Content>` |
| AlertDialog | `overlays/alert-dialog.tsx:22-26` | 16 as `AlertDialog`, plus 5 imported under the alias `Alert` |
| Popover | `overlays/popover.tsx` | 3: `TabRename.tsx:134`, `ChatsSection.tsx:1480`, `PanelList.tsx:472` |
| Tooltip | `overlays/tooltip.tsx` | 5: `StatusLine.tsx:318`, `settings/components.tsx:449` and `:481`, `settings/ColumnHeading.tsx:47`, `primitives/MenuRow.tsx:94` |
| DropdownMenu | `overlays/dropdown-menu.tsx` | 5: `AwaySummary.tsx:179`, `StripMarks.tsx:267`, `TabChip.tsx:583`, `VaultTab.tsx:1023`, `settings/SettingsTab.tsx:1191` |
| ContextMenu | `overlays/context-menu.tsx` | 2: `Menus.tsx:92` (`Menued`, 22 call sites in 14 files) and `TerminalMenu.tsx:102` (1 site) |
| Select | `overlays/select.tsx` | 1: `primitives/Select.tsx:82` |

In total there are 43 files that import the dialog wrappers and 50 dialog contents. The 51st dialog is the theme specimen's static `.ui-dialog` at `specimen/Specimen.tsx:243`.

**The surface** (`primitives/overlays.css:28-61`). One rule draws `.ui-dialog`, `.ui-popover`, `.ui-tooltip` and `.ui-menu`.

| Part | Value | Token or literal |
|---|---|---|
| border | `1px solid var(--border-subtle)` | width is a literal |
| corner | `var(--radius-lg)` | token |
| ground | `--surface-overlay` | token |
| lift | `--elevation-overlay` | token |
| dialog and popover padding | `--space-6` | token |
| dialog bottom padding | `--ui-dialog-padding-end` | the one hook a view may set (`:48`) |
| tooltip padding | `--space-1` / `--space-3` | token |
| tooltip size | `--font-size-sm` | token |
| popover size | `--font-size-md` | token |
| arrow | `.ui-overlay-arrow` fill `--surface-overlay` | token |
| scrim | `.ui-scrim` `--overlay-scrim` (`:98`) | token |

- The inline surface `.ui-inline-surface` (`:76`), used for notices, has `--radius-md` and no lift.
- All 50 dialogs use `ui-scrim`, with no exceptions.
- The open animation is `App.css:7292-7295`, and it plays on open only.

**What a view still hard-codes on overlays.**

*Placement and size live in `App.css`, in rem/vh/vw literals and not tokens:*
- `.warning` (`App.css:4279-4287`) is the base placement of 48 of the 50 dialogs: fixed, centred by `translate(-50%,-50%)`, `max-width: 34rem`, `max-height: 80vh`, `overflow: auto`. It sets **no width**, so each dialog is as wide as its content, up to 34rem.
- The per-dialog overrides use 7 different widths and 2 height caps:

| Override | Width | Height cap |
|---|---|---|
| `.ask-persona` (`App.css:4301`) | `min(34rem, 100vw-2rem)` | `calc(100vh-2rem)` |
| `.brief-panel` (`App.css:4315`) | `min(44rem, 100vw-2rem)` | `calc(100vh-2rem)` |
| `.answer-panel` (`App.css:4326`) | `min(44rem, 100vw-2rem)` | `calc(100vh-2rem)` |
| `.starting` (`App.css:4421`) | `min(38rem, 92vw)` | base 80vh |
| `.palette` (`App.css:4782`) | `min(42rem, 92vw)`, `top: 8vh` | base 80vh |
| `.update` (`App.css:5443`, About too) | `max-width: 40rem` | base 80vh |
| `.doctor` (`App.css:5470`) | `48rem`, `width: calc(100vw - 4rem)` | base 80vh |

- The window margins are written 3 ways: `2rem`, `92vw` and `4rem`.
- Classes that no rule styles: `.extensions`, `.task-end` (the dialog) and `.about` exist only as names.

*Popovers:*
- `.row-card` (`App.css:6097`) is `max-width 24rem` and `max-height 28rem`.
- `.chat-card` (`App.css:6115`) is `max-width 20rem`.

*Menus:*
- The menu width defaults to `calc(--space-8*6)` and `calc(--space-8*10)` (`primitives/controls.css:487-488`), which are token-derived.
- `.tasks-menu` uses `min(30rem, 90vw)`, a literal (`App.css:2076`).
- `.more-menu` has no maximum (`App.css:824`).
- `.ui-setting-move` sets `space-8*7` (`App.css:8294`).

*Tooltips:*
- `.ui-menu-tip` is `space-8*9` (`controls.css:620`).
- `.ui-setting-tip` is `space-8*10` (`App.css:8256`).
- The plain `.ui-tooltip` has **no maximum width**, and it is the one used for the status bar's project path (`StatusLine.tsx:318`).

*z-index literals:* `.ui-menu` 10 (`controls.css:496`), `.ui-menu-tip` 20 (`controls.css:621`), `.ui-setting-tip` 20 (`App.css:8258`). `#root{isolation:isolate}` (`App.css:72`) is what keeps every overlay above the window.

*Offsets and arrows vary per site:*
- `sideOffset` is 2 (Select), 4 (most), 6 (PanelList popover), 8 (MenuRow tip), or Radix's default 0 (`ChatsSection.tsx:1480`).
- Arrows: drawn on 4 tooltips and 1 popover (`PanelList.tsx:485`). Missing on the menu-row tip (`MenuRow.tsx:94`) and on 2 popovers (`TabRename.tsx:134`, `ChatsSection.tsx:1480`).

*Tooltip delays:* 300ms on 4 Radix providers (`StatusLine.tsx:302`, `settings/components.tsx:441` and `:471`, `ColumnHeading.tsx:38`), and 600ms on menu rows (`primitives/MenuRow.tsx:21`).

*Two tooltip systems:* 5 Radix tooltip sites against **219 native `title=` attributes in 85 files**. A native title tooltip is drawn by the OS: a theme can't style it, and its delay is the OS's.

*Menu rows:* every menu item uses `.ui-menu-item` (`controls.css:528`): a control-tall row, `--font-size-md`, `--radius-md`, highlight `--tab-active`. There are 19 raw `Menu.Item` and `ContextMenu.Item` sites. The helpers in `primitives/MenuRow.tsx` (`MenuWords`, `MenuTip`, `MenuKeys`) are used in 4 files.

### B.2 Dialog action rows: two primitives, mirrored

| | `AnswerBar` (`AnswerBar.tsx:27`) | `SettingActions` (`settings/components.tsx:983`) |
|---|---|---|
| Call sites | 32 in 28 files | 56 in 39 files, inside and outside dialogs |
| Class | `.answer` | `.ui-setting-actions` |
| CSS | `App.css:4432` | `App.css:8380` |
| Justify | `flex-end` (trailing edge) | start, wraps (leading edge) |
| Order | way out first, then the act | the act first, then Cancel |
| Used for | questions | forms |

The order rule is written down in #1719 and `docs/design-system.md`. The effect on screen is that a "New vault" dialog has its buttons at the left reading [Create][Cancel], while "Delete vault" has them at the right reading [Cancel][Delete]. Three dialogs hold both kinds of row:
- Extensions: `Extensions.tsx:221` and `:353`.
- Updates: `Updates.tsx:419` and `:448`.
- Save: `SavingView.tsx:303/424` and `:635`.

**Duplicated button rules.**
- `.answer button` (`App.css:4441`) and `.ui-setting-actions button` (`App.css:8406`) both restyle buttons: padding `space-2/space-4`, border `--border-subtle`, and different hovers (`--border-subtle` and `--control-hover`).
- For `.ui-button` these are **dead**, because the `components` layer wins: `.ui-button` has `--size-control` height and border `--border-strong`.
- They are still live for the buttons in a row that have no `ui-button` class:
  - `ChangeActions.tsx:67` and `:72`: the Cancel and Push/Land buttons of the "Push {change}?" and "Record/Land" dialogs. These draw shorter, with a padding-based height and a subtler border, than every other dialog's buttons.
  - Doctor's `.doctor-fix` and `.doctor-settings` buttons (`Doctor.tsx:393`, `:593`). They are classed, so the `base.css:34` token tier skips them, and no rule gives them a look (`App.css:5539` only places them). WebKit's native button probably shows through. **Check this on screen.**

### B.3 Every dialog

In the table, a footer "scrolls away" means the action row is inside the scrolling `.warning` box (`overflow:auto`, 80vh) with no sticky footer. **Only Ask {persona} has a sticky footer** (`App.css:4383`). **Brief keeps its buttons by being a flex column** (`App.css:4315`, where the brief box `.brief-text` at `:4346` is the part that scrolls). Every other dialog's buttons scroll away once its content passes 80vh.

| # | Dialog (title) | Content at | Wrapper | Width rule | Max-height | Action row | Buttons scroll away? | Long lists inside |
|---|---|---|---|---|---|---|---|---|
| 1 | About purlis | `About.tsx:108` | Dialog | `.update` 40rem max | 80vh | AnswerBar `:161` | rarely: the body is capped | release notes `.about-body` 24rem inner scroll (`App.css:5912`) |
| 2 | Add to a chat ({label}) | `AddToAChat.tsx:42` | Dialog | base 34rem | 80vh | AnswerBar | no (short) | none |
| 3 | Answer {task} | `AnswerPanel.tsx:85` | Dialog | `.answer-panel` `min(44rem,100vw-2rem)` | `100vh-2rem` | the question form's own row | **yes** | form fields |
| 4 | Trust the extension? | `ApproveExtension.tsx:71` | Dialog | base | 80vh | AnswerBar `:111` | **yes**, below the list | `ul.contributes` `:93` unbounded |
| 5 | Open this project? / changed since | `ApprovePlane.tsx:79` | Dialog | base | 80vh | AnswerBar `:227` | **yes** | 6 unbounded lists (`changes` `:107`, `contributes` at `:125/140/157/179/194`) |
| 6 | Ask {persona} | `AskPersona.tsx:119` | Dialog | `min(34rem,100vw-2rem)` | `100vh-2rem` | SettingActions, **sticky** | **no** | workspace select, radio |
| 7 | Brief of {task} | `Brief.tsx:276` | Dialog | `min(44rem,…)`, flex column | `100vh-2rem`, hidden | AnswerBar `:343` | **no** | brief text box inner scroll |
| 8 | Push {change}? | `ChangeActions.tsx:167` | AlertDialog | base | 80vh | AnswerBar with **unclassed buttons** | **yes** | `ul.saving-files` `:192` unbounded |
| 9 | Record / Land {repo}'s {request} | `ChangeActions.tsx:289` | AlertDialog | base | 80vh | same as #8 | **yes** | `ul.came-back` `:98` |
| 10 | Chat ask ({title}) | `ChatAsk.tsx:60` | AlertDialog | base | 80vh | AnswerBar | no | none |
| 11 | Closing project ({title}) | `ClosingProject.tsx:64` | AlertDialog | base | 80vh | AnswerBar | no | inline list of workspaces |
| 12 | Delete vault {vault}? | `DeleteVault.tsx:82` | AlertDialog | base | 80vh | AnswerBar `:134` | **yes** when the vault has many secrets | `ul.at-risk` `:106` unbounded |
| 13 | Delete workspace {ws}? | `DeleteWorkspace.tsx:92` | AlertDialog | base | 80vh | AnswerBar `:154` | **yes** | `ul.at-risk` `:137` unbounded |
| 14 | Doctor | `Doctor.tsx:686` | Dialog | `.doctor` 48rem / `100vw-4rem` | 80vh | AnswerBar `:791` | **yes, always**: about 20 rows | `ul.doctor-rows` `:578` unbounded, plus 2 `<details>` |
| 15 | {offer.title}? (end or smart-close a chat) | `EndingChat.tsx:152` | AlertDialog | base | 80vh | AnswerBar `:245` | no | drafts named in a label |
| 16 | Run "{action}" from {ext}? | `ExtensionAction.tsx:94` | AlertDialog | base | 80vh | AnswerBar | no | none |
| 17 | Extensions | `Extensions.tsx:191` | Dialog | base (`.extensions` has no rule) | 80vh | SettingActions `:221` at the top, AnswerBar `:353` at the bottom | **yes** | `extension-rows` `:260` and 2 `contributes` lists, unbounded |
| 18 | Sign in with GitHub | `ForgeDeviceSignIn.tsx:121` | Dialog | base | 80vh | SettingActions `:149`, `:201` | no | short |
| 19 | Sign in with GitLab | `ForgeGitLabSignIn.tsx:95` | Dialog | **none**: `className="ui-dialog"` only | **none** | SettingActions | n/a | none |
| 20 | Sign in with a token | `ForgeTokenSignIn.tsx:119` | Dialog | base | 80vh | SettingActions | no | scopes list (short) |
| 21 | Link to work item | `LinkWorkItem.tsx:58` | Dialog | base | 80vh | SettingActions | no | none |
| 22 | Make {ws} public / private? | `LiveDialog.tsx:117` | AlertDialog | base | 80vh | AnswerBar ×2 | **yes** | `ul.at-risk` `:138`, the files it publishes, unbounded |
| 23 | New branch | `NewBranch.tsx:57` | Dialog | base | 80vh | SettingActions | no | none |
| 24 | New persona | `NewPersona.tsx:88` | Dialog | base | 80vh | SettingActions `:172` | no | select |
| 25 | New project | `NewProject.tsx:137` | Dialog | base | 80vh | SettingActions `:194`, `:334` | sometimes | 2 `<details>` |
| 26 | New vault | `NewVault.tsx:70` | Dialog | base | 80vh | SettingActions | no | provider radio (fixed, short) |
| 27 | New workspace | `NewWorkspace.tsx:80` | Dialog | base | 80vh | SettingActions `:181` | sometimes | RepoPicker `.repo-picks` 14rem inner scroll (`App.css:5731`) inside the outer scroll |
| 28 | Palette (no title) | `Palette.tsx:496` | Dialog | `.palette` `min(42rem,92vw)`, top 8vh | 80vh | no buttons | n/a | **two** listboxes, each `max-height 55vh` (`App.css:4808`): together up to 110vh inside an 80vh box, so a scroll sits inside a scroll |
| 29 | Profile of {persona} | `PersonaProfile.tsx:68` | Dialog | base | 80vh | SettingActions `:141`, `:164` | **yes** with many profiles | profile radio `:124` unbounded |
| 30 | Publish to GitHub or GitLab | `PublishProject.tsx:117` | Dialog | base | 80vh | SettingActions `:129`, `:218` | sometimes | none |
| 31 | Quitting ends N chats | `QuitWarning.tsx:80` | AlertDialog | base | 80vh | AnswerBar `:104` | **yes** with many chats | `EndingList` `ul.ending` `:140` unbounded |
| 32 | Restart mid-turn ({title}) | `QuitWarning.tsx:239` | AlertDialog | base | 80vh | AnswerBar `:252` | no | none |
| 33 | Reopen your sessions? | `RelaunchAsk.tsx:57` | AlertDialog | base | 80vh | AnswerBar | **yes** with many sessions | `ul.ending` `:69` unbounded |
| 34 | Remove {repo} from {ws}? | `RemoveFromWorkspace.tsx:69` | AlertDialog | base | 80vh | AnswerBar | no | none |
| 35 | Delete persona {p}? | `RemovePersona.tsx:48` | AlertDialog | base | 80vh | AnswerBar | no | none |
| 36 | Rename workspace | `RenameWorkspace.tsx:56` | Dialog | base | 80vh | SettingActions | no | none |
| 37 | Merge the Nth of {x}? | `SavingMerge.tsx:285` | AlertDialog | base | 80vh | AnswerBar | **yes** | failed checks `ul.saving-files` `:241` |
| 38 | Save all ({title}) | `SavingView.tsx:615` | AlertDialog | base | 80vh | AnswerBar `:635` | **yes** | `ul.saving-files` `:627` unbounded |
| 39 | **Start a chat** | `StartChat.tsx:276` | Dialog | `.starting` `min(38rem,92vw)` | 80vh | SettingActions about `:563`, **last** | **yes**: the known case | Harness radio, then Persona radio (`Choice kind="radio"`, no cap anywhere), refused `<details>` `:501`. With many personas, Start and Cancel go below the fold. |
| 40 | End task ({name}) | `TaskEnd.tsx:154` | AlertDialog | base (`.task-end` has no rule) | 80vh | AnswerBar `:214` | no | short |
| 41 | Replace {folder}? | `ThemeDraftStrip.tsx:212` | AlertDialog | base | 80vh | AnswerBar | no | none |
| 42 | Who draws {folder}? | `ThemeDraftStrip.tsx:331` | AlertDialog | **none**: `className="ui-dialog"` only | **none** | AnswerBar | n/a | none |
| 43 | Updates | `Updates.tsx:335` | Dialog | `.update` 40rem | 80vh | SettingActions `:419` and AnswerBar `:448` | sometimes | notes `.update-notes` 12rem inner scroll (`App.css:5448`) |
| 44 | The project's pin | `Updates.tsx:560` | Dialog | `.update` | 80vh | AnswerBar `:572` | no | none |
| 45 | How {vault} signs in | `VaultTab.tsx:565` | Dialog | base | 80vh | the sign-in form's own row | sometimes | provider form |
| 46 | Add or edit secret ({title}) | `VaultTab.tsx:1114` | Dialog | base | 80vh | SettingActions `:1163` | no | none |
| 47 | Rename {secret} | `VaultTab.tsx:1228` | Dialog | base | 80vh | SettingActions `:1254` | no | none |
| 48 | Delete {secret} from {vault}? | `VaultTab.tsx:1310` | AlertDialog | base | 80vh | AnswerBar `:1328` | no | none |
| 49 | Open vault | `Vaults.tsx:162` | Dialog | base | 80vh | AnswerBar | no | `PanelList` `.panel-rows` 14rem inner scroll (`App.css:5989`) |
| 50 | Remove / Forget profile | `settings/profiles.tsx:373` | AlertDialog | base | 80vh | AnswerBar `:388` | **yes** with many conversations | `ul.at-risk` `:381` unbounded |
| (51) | Specimen dialog | `specimen/Specimen.tsx:243` | static `div role=dialog` | `.warning` | 80vh | AnswerBar | n/a | none |

**Totals.**

*Action rows:*
- 2 of 50 dialogs keep their actions in reach while content scrolls: #6 (sticky) and #7 (flex column).
- About 20 can push the actions out of view when their content is long: #3, 4, 5, 8, 9, 12, 13, 14, 17, 22, 29, 31, 33, 37, 38, 39, 50, and sometimes 25, 27, 30, 43, 45.

*Long lists:*
- 5 lists are capped with their own inner scroll: about-body, update-notes, repo-picks, panel-rows, and palette-rows.
- Every other list in a dialog is unbounded: at-risk, saving-files, contributes, changes, extension-rows, doctor-rows, ending, and radio `Choice` lists.

*Placement:*
- **2 dialogs have no placement rule**: `ForgeGitLabSignIn.tsx:96` and `ThemeDraftStrip.tsx:332` (`KeepAsk`). Without `.warning` they are not fixed, centred or capped, and Radix portals them to the end of `<body>` in normal flow. They probably draw off-screen or at the window's bottom-left. **Check this on screen.**

*Outside clicks:*
- 21 of the 29 `Dialog`s block outside clicks with `onInteractOutside`.
- The 8 that close on an outside click are About, AddToAChat, AnswerPanel, Brief, Doctor, Extensions and both Updates dialogs.
- AlertDialogs never close on an outside click.

*Other gaps:*
- No dialog has a corner × close button: `Dialog.Close` appears 10 times, as text buttons.
- Secondary text in dialogs is dimmed with **literal opacity**, not a text token: `.honest` 0.8 (`App.css:4427`), `.palette-ask` 0.7 (`:4794`), `.ending .what` 0.7 and `.ending .where` 0.55 (`:4405`, `:4411`).

---

# 3. Settings forms

### C2. Settings form rows: alignment

- **The label has no column.** `.ui-setting-name` is `flex: 0 1 auto; max-inline-size: 40%` (App.css:8221-8229), so it is as wide as its own words. The comment there records that fixed bases were tried and made rows wrap with both sidebars open. As a result, **each row's control starts at a different x**: label/control/Reset make a ragged left edge on every settings page. A two-column grid (`subgrid` on the group, or `grid-template-columns: minmax(auto, 40%) 1fr auto`) would align controls and still wrap below a container width.
- **Reset, Undo and Move to… are in the control's own flex** (`.ui-setting-control`, App.css:8298-8305, `flex-wrap`, gap `--space-4`, `min-inline-size: calc(--space-8 * 6)`). They follow the control, so they line up only when two controls have the same width. Reset appears only when the value is set at this level (SettingsTab.tsx:963-971, `mayTakeOut`), so in a run of rows the Move to… menu **jumps left or right by a Reset's width**.
- **Reset is bordered and Move to… is ghost** in the same row (components.tsx:379 vs SettingsTab.tsx:1182).
- **Grouped rows** (a checks or status control) drop the control to its own line (`.ui-setting-row-grouped .ui-setting-name { flex-basis: 100% }`, App.css:8231). The same applies to list fields (App.css:8323) and whole-list controls (App.css:8316). So within one group, some rows are side by side and some are stacked.
- **Group help comes once, under the h3** (components.tsx:241-245, DS-8). Per-row help is hidden behind the info mark on settings pages but drawn in dialogs. This is consistent, but people have to hover to learn what a row does.

### C3. Settings pages and the long ungrouped lists

The groups per level come from settings/catalogue.ts:23-60. **You: 10 groups** (Text, Theme, Density and fonts, Editor, Chats list, Notifications, This machine, Browsers, Accounts, Profiles). **Project: 14 groups**, 4 of them sub-pages under Sandbox (Your hosts, Your level, Persona hosts, Network). **Workspace: 7 groups**. The Persona level has no groups yet. The rows inside a group come from the core's file declarations (`FileSetting`), so they can't be counted statically from `app/src`. Groups also carry `after:` free-form ReactNode rows (e.g. Sandbox: expects rows, persona-hosts rows, sandbox reasons; settings/project.ts:614-619) that don't go through `SettingRow`.

**Your hosts** (`project.sandbox.mine`, settings/project.ts:638-645) and the project's **hosts** (`project.sandbox`, project.ts:613) use `hostsCollection` (project.ts:274-298). Every entry is drawn by Collection.tsx:171-220 as:
- a `ui-collection-entry` block with `margin-top: --space-5` (12px) per host;
- an `<h4>` showing the host (bold, 12px), followed inline by **Confirm** (only while `confirmed = "no"`, project.ts:285) and **Remove**, both small `.ui-setting-reset` buttons;
- a Remove confirm that opens a whole indented form under the entry (Collection.tsx:221-249).

There is no ordering, no split between unconfirmed and confirmed, no bulk Confirm or Remove, no search, and no count. With 20 hosts this is 20 headings, each carrying its own two buttons, and their x-position follows the host's length. Where structure would fit:
- **A table** (`.ui-table` already exists and stacks under 29rem): Host | Status (Unconfirmed / Confirmed) | actions in a fixed trailing column. This fits Your hosts, Persona hosts, the project hosts, profiles, the Network granted list (GrantedList.tsx:160-209: sentence, notes, Remove) and This machine.
- **Group by state**: "Waiting for your Confirm (n)" above, with a single Confirm-all, and confirmed below, collapsed. yourLevel.tsx:245 already has the label "Waiting for your Confirm".
- **Collapse** the long help and notes per row. GrantedList repeats "Removing it edits the committed {file}." on every project-level row (GrantedList.tsx:181-183); say it once in the group head.
- **Tabs or segments** inside the Sandbox group, which is already split into 4 nav sub-pages. Network alone mixes Open hosts, allowed hosts and folders, and recently blocked (GrantedList.tsx:360-377).
- The **Dispatch grants** table (DispatchGrants.tsx, 1,553 lines; one row group per persona, App.css:8042-8081) is the one long list that is already a table. It is the model for the others.

---

# 4. Typography

### C4. Typography

#### The scale that exists (`app/src/theme/geometry.css:41-59`; themable within bounds through `crates/purlis-core/src/theme/geometry.rs:96-160`, see 6.2)

| Token | Value at 14px | Weight / line tokens |
|---|---|---|
| `--font-size-2xs` | 10px | `--font-weight-regular` 400 |
| `--font-size-xs` | 11px | `--font-weight-medium` 500 |
| `--font-size-sm` | 12px | `--font-weight-strong` 600 |
| `--font-size-md` | 13px | `--line-height-tight` 1.2 |
| `--font-size-base` | 14px (1rem) | `--line-height-body` 1.4 |
| `--font-size-lg` | 16px | `--font-mono` |
| `--font-size-xl` | 20px | `--font-ui` |

Tailwind's `text-*` and `rounded-*` are bound to these (styles.css:244-257). In tsx, Tailwind text utilities are used 5 times (`text-sm` ×4, `text-xs` ×1).

#### What is actually used (every `font-size:` in `app/src/**/*.css`, 139 declarations)

| Value | Count | | Weight | Count | | Line-height | Count |
|---|---|---|---|---|---|---|---|
| xs (11px) | **73** | | strong 600 | 44 | | `1` (literal) | **9** |
| sm (12px) | 36 | | regular | 9 | | body token | 5 |
| md (13px) | 12 | | medium | 3 | | tight token | 2 |
| base (14px) | 8 | | inherit | 1 | | inherit | 1 |
| inherit | 4 | | *UA bold 700 via unweighted h2/h3/h4* | **7 rules** | | | |
| xl | 2 | | | | | | |
| lg | 2 | | | | | | |
| 2xs | 1 | | | | | | |
| `14px` literal | 1 (geometry.css:26, the root) | | | | | | |

By file: App.css 121, controls.css 7, text.css 6, overlays.css 2, collections.css 2, geometry.css 1. **No literal px sizes** outside the root; the tokenisation work (#1920) held. What remains is **how the tokens are ranked**, not whether they are used:
- **xs is the workhorse (73 of 139).** Half the app's text is 11px, one step above the floor.
- Line-height `1` is a literal in 9 places (App.css:520, 557, 892, 918, 1610, 2006, 6863; controls.css:50, 303), with no token for it.
- `letter-spacing: 0.06em` + uppercase is written **4 times** (text.css:59-60 `.ui-text-label`; App.css:3537 `.sidebar-title`; App.css:6519 `.dispatch-read h3`; App.css:6688 `.session-dispatches h3`), plus a 5th variant without uppercase (`.inbox-chain`, App.css:2951). `.sidebar-title` and `.ui-text-label` have identical values and are worn together (Explorer.tsx:1230, 1242; ChatsSection.tsx:853; PanelSection.tsx:59).

#### The text roles (primitives/text.css) and their adoption

| Role | Size / weight / colour | tsx uses | files |
|---|---|---|---|
| `.ui-text-title` | lg / strong / primary | **0** | 0 |
| `.ui-text-heading` | base / strong / primary | **1** | 1 (ChatNetworkTab) |
| `.ui-text-body` | md / regular / primary | 27 | 13 |
| `.ui-text-secondary` | sm / regular / secondary | 46 | 18 |
| `.ui-text-meta` | xs / regular / muted | 25 | 10 |
| `.ui-text-label` | xs / strong / secondary / uppercase | 12 | 8 |
| `.ui-text-code` | mono | 7 | 5 |
| `.ui-text-strong` | strong | 15 | 8 |

The roles cover rows and their meta. **No heading in the app wears a heading role.** There are 69 `<h1-h4>` in tsx (h1 2, h2 17, h3 43, h4 7) and 22 `<strong>`. Each heading is sized by its own App.css rule, and these differ: `.view-head h2` base (App.css:6277), `.warning h2` base (4291), `.welcome h2` md (4939), `.warning h3` md (5227), `.ui-setting-group h3` md (7973), `.warning.doctor h3` sm (5477), `.ui-collection-head h4` sm (8178), `.dispatch-read h3` xs uppercase (6517), `.session-dispatches h3` xs uppercase (6686), `.warning h4` xs regular (5232). **7 of those rules set no weight**, so they render at the UA's 700 instead of the 600 token: App.css:4289, 5225, 5475, 6270, 7971, 8176. `<strong>` (22 uses) is also UA 700.

#### Where secondary text is louder than primary, or the reverse

1. **The Inbox (the needs-you panel). The chat's name is the quietest text in its group.** The group heading `h3.inbox-chain` is xs (11px), strong, `--text-secondary`, letter-spaced (Inbox.tsx:427; App.css:2949-2955). Each ask's line under it is `ui-text-body ui-text-strong` (13px, 600, primary; Inbox.tsx:820). For a chat that only waits, that line is the generic status. "Waiting on your reply" is the `question` source's sentence (Inbox.tsx:639), shown in the meta (Inbox.tsx:830-833, xs muted). The status therefore outranks the name by two sizes and a colour step. **InboxElsewhere does the opposite**: the project name is the primary line, `${project} · N waiting on you` (InboxElsewhere.tsx:114). Seen every time the person opens the Inbox or the title-bar hand (NeedsYou.tsx:232-243 opens it). Very high frequency.
2. **Settings rows: the control's text is larger than its label and the group heading.** View tabs render inside `.pane.view` with `font-size: --font-size-sm` (12px; App.css:6242-6249; LayoutPanes.tsx:475). The row label inherits 12px at medium weight (App.css:8236). The group's h3 is 13px (App.css:7973). But `.ui-input` and `.ui-button` set `--font-size-base`, 14px (controls.css:49, 178). The **value outshouts both the label and the heading** on every settings row. The same happens in every dialog form that uses `SettingRow`.
3. **Buttons are a size above the text around them in every view pane.** `.ui-button` is 14px inside 12px panes. `.ui-button-sm` is 12px (controls.css:116) inside `.notice-pane-box`, whose text is 11px (App.css:2311). So the Notice's buttons outweigh the Notice's sentence.
4. **The same Notice at two sizes**: `.notice-pane-box` xs (App.css:2311) vs `.notice-inbox` sm (App.css:4223).
5. **The Explorer section head**: the label is 11px uppercase (`ui-text-label`, Explorer.tsx:1242-1247), and its "of" suffix is `ui-text-secondary` at 12px (Explorer.tsx:1259; transform reset in App.css:3563-3568). **The qualifier is bigger than the title it qualifies.** The same pattern applies to `.panel-from` in panel heads (PanelSection.tsx:66, App.css:5977-5980).
6. **Group headings are drawn four ways at one rank**: `.sidebar-title`/`.ui-text-label` (xs strong secondary uppercase), `.inbox-chain` (xs strong secondary, spaced, not uppercase), `.panel-day` (xs strong **muted**, App.css:5996-6000; the text-role rule says muted is the meta's alone, text.css:13), and `.chats-group` (`ui-text-label`, ChatsSection.tsx:1002). **The workspace name in the Chats list is uppercased** by the label role, while the Inbox comment says a chain "is data, so it is drawn as written" (App.css:2946-2948).
7. **The Tasks menu**: an ended task's report is md (13px; App.css:2157), larger than the row's own where/since/asked (sm; App.css:2145, 2170), and as large as the task name. `.tasks-menu-row .doing` is commented as "one dim line" but sets **no colour** (App.css:2197-2205), so it is primary text.
8. **The scope switch's counts are muted at 10px** (`.ui-level-count`, App.css:2925; `ui-levels-small`). This is the only use of `2xs`, and it is on the first control of the Inbox.
9. **Chat rows are consistent**: name `ui-text-body` (ChatsSection.tsx:1452), state word sm secondary (App.css:4546), task count `ui-text-meta ui-text-strong` (1599). Also consistent: PanelList rows (body, strong for the default tone, then meta note; PanelList.tsx:415-421) and the Explorer repo (body strong, Explorer.tsx:1047). These are the models to copy.
10. **The title bar** is xs (App.css:5804), while its project strip is base (5819). Its height is a literal `min-height: 2.1rem` (App.css:5806), not `--size-chrome` or `--size-control`.

Not verified from `app/src` (it lives in the core, which wasn't fetched): the exact `says` text the registry gives a chat that only waits, and so whether the primary line reads "Waiting on your reply" or the chat's last question. The ranks above hold either way.

---

# 5. Button sizing: every row whose buttons differ

### A.5 Button sizing: rows where adjacent buttons differ (item 5)

Heights are computed at 14px.

1. **Title bar row** (`TitleBar.tsx:174-195`, always on screen). Left to right:
   - `NeedsYouButton`: `.needs-you-button`, ~15px, borderless, no ground.
   - `SaveIndicator` (`SavingView.tsx:799,820,833`): `ui-button ui-button-ghost ui-button-sm`, 21px, `--font-size-sm`, borderless.
   - `SandboxMark`.
   - `KillSwitch`: `.title-kill-switch`, ~17px, bordered, xs, secondary text.
   - `About`: `.title-about`, ~17px, bordered, xs, **secondary** text, icon + "About".
   - `UpdateItem` (`Updates.tsx:315`): `.status-update`, ~17px, bordered, xs, **primary/muted/accent** text, icon alone when quiet, icon + words when offered.

   That is three button families, three heights (15/17/21px) and two font sizes (xs vs sm) in one row. The comment at App.css:5849 claims "one row, one shape", but About and the update item only share chrome by copy-paste, and their text colour differs: About is `--text-secondary`, a quiet update is `--text-muted`, an offered one is accent. A quiet update is an icon-only bordered box beside a labelled bordered box.
2. **Status bar** (`StatusLine.tsx:189-400`, always on screen). `status-alerts` (`:361,379`), `status-doctor` (`Doctor.tsx:664`) and `status-pin` (`Updates.tsx:548`) are all `ui-button-ghost ui-button-sm` at 21px, `--font-size-sm`. `status-project` (`StatusLine.tsx:306`) is a focusable `<span>`, not a button. The status bar's own text is `--font-size-sm` (App.css `.status-line`) and the title bar's is `--font-size-xs`, so a chrome button is 12px text at the bottom of the window and 11px at the top.
3. **Notice action row** (`Notice.tsx:185-243`, frequent: every inbox/pane/window notice). "Go to it", fixes, the link and "Copy command" are bordered `ui-button-sm`. **"Dismiss" is `ui-button-sm ui-button-ghost`**, borderless. A grants fix adds an accent edge and a lock. Because the row is a `.ui-button-row` (`controls.css:150`), a long fix label such as **"Update workspace layout"** (`InboxAlerts.tsx:199`) wraps in a narrow pane and grows past 21px while "Dismiss" stays 21px. The result is two heights and two styles in one row.
4. **Settings row** (`settings/components.tsx:374-407`, on every setting). The control (28px), then "Reset"/"Undo" (`ui-setting-reset ui-button`: secondary, bordered, 28px), then **"Move to…"** (`SettingsTab.tsx:1182`: `ui-button-ghost`, borderless, 28px). The two adjacent buttons do the same kind of job in different styles.
5. **Settings collection rows** (`settings/Collection.tsx:145-212`, e.g. Your hosts). Open/Rename/Confirm/Remove use raw `.ui-setting-reset`: ~26px, subtle border, `cursor: pointer`. They sit on the same page as Reset (28px, strong border). Their confirm row (`Collection.tsx:236-246`, Remove/Cancel; `:579-583` Add/Cancel; `:673-682` Rename/Rename everywhere/Cancel) uses unclassed buttons, so they take `.ui-setting-actions button` at ~30px, subtle border, opacity .45 when disabled. That makes three button heights (26/28/30) on one settings page.
6. **Dialog footers have two orders and two looks.**
   - `AnswerBar` (`AnswerBar.tsx:28`, `.answer`, `justify-content: flex-end`, no wrap; 32 uses) puts the way out first and the answer last (21 of 32 start with Cancel/Wait).
   - `SettingActions` (`settings/components.tsx:983`, `.ui-setting-actions`, start-aligned, wraps; 56 uses) puts **primary first and Cancel last** (25 of 56 end with Cancel/Close), for example `NewVault.tsx:133` and `NewWorkspace.tsx:181`. That contradicts the comment at `TaskEnd.tsx:308`: "the answer bar's order, which every question keeps".
   - In these rows the buttons are tier-2/3 unclassed (~30px, subtle border, no primary emphasis) rather than `.ui-button` (28px, strong border): `GetProject.tsx:205`, `NotCloned.tsx:59,132`, `FirstRun.tsx:193,220`, `ImportCliLogin.tsx:100` ("Not now" | "Use my login": no primary, and in reversed order), `MakeItAProject.tsx:131` (same), `FirstTaskTab.tsx:371`, `VaultSignIn.tsx:328,476`, `ForgeQuestion.tsx:37`, `GitIdentityForm.tsx:127`, `CreateOnForge.tsx:329,416`, `Welcome.tsx:521`, `WorkspaceRepos.tsx:127` (raw `panel-view` ×2 → 17px/xs inside a 30px-button container type), `WorkspaceRepos.tsx:221`.
   - `ChangeActions.tsx:65` is the one `AnswerBar` with unclassed buttons (`.answer button`, ~30px subtle) where the other 31 use `.ui-button` (28px strong).
7. **Panel answer row** (`AnswerQuestion.tsx:213-221`, `.answer-actions`). Submit and Cancel are raw `.panel-view`: 17px, xs, radius-sm. The same question in a dialog uses 28px `ui-button`s. Its textarea is `.ui-field`, not `.ui-input`.
8. **Inline task-end question** (`TaskEnd.tsx:310-315`, `.task-end-confirm button`). "Keep" and the answer are 21px, xs, on the raised ground with no primary. The full TaskEnd dialog (`TaskEnd.tsx:214-239`) uses 28px Cancel / danger "Close now" / primary "Stop and get its report". The same decision gets two sizes and two emphases.
9. **Inbox row** (`Inbox.tsx:773, 857-874, 1337-1379`). The answers (`.inbox-answers ui-button-row`) are `ui-button-sm` bordered, and the acts (`.inbox-acts`) are `ui-button-ghost ui-button-sm` (`inbox-go`). Consistent within each row, but two styles stack on one row block. `Inbox.tsx:963` is a full-size 28px `ui-button` among the sm ones.
10. **Sandbox block notice** (`SandboxBlockNotice.tsx:273-287, 411-433, 972-978`): primary-sm / sm / grants-sm side by side. These are deliberate variants with matching sizes. The other noted cases are not.
11. **Explorer section head** (`Explorer.tsx:595-697`): `ui-button-sm` toggles sit beside the `.explorer-section-head > button` header (App.css:3559) and `.needs-you-mark.rolled-up` (padding `--space-1`, radius-sm).
12. **Welcome / onboarding** (`Welcome.tsx:459,522,551`, `Onboarding.tsx:189,296,305`): `--radius-lg` and `--space-5` padding make these the only rounded-lg buttons in the app. `.welcome-link` (×7) and `welcome-door` sit beside them.
13. **Tab chip** (`TabChip.tsx:501,524,1158`): pill-shaped (`--radius-full`) 21px buttons inside a tab whose own corners are `border-radius: 0` (App.css:441).
14. **Strip marks** (`StripMarks.tsx:99,254,361,395`): a `ui-button-primary` (28px) in the same strip as `.show-more` (padding `--space-1`, borderless), `.closer` and `.gear` (radius 0, padding 0).

---

# 6–7. Theme reach and extension seams

### 6. What a theme can and cannot change today

#### 6.1 The token vocabulary (the engine's: `crates/purlis-core/src/theme/vocabulary.rs`)

| Kind | Count | Where | Bounds / grammar |
|---|---|---|---|
| Colour tokens | **119** | `vocabulary.rs:15-135` (`TOKENS`); mirrored in `app/src/theme/theme.ts:59ff` | hex only: `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`, or a palette name (`vocabulary.rs:304-309`, `schema.rs:31`). 6 tokens may also be `"platform"` (`PLATFORM_DRAWN`, `vocabulary.rs:388-395`) |
| Flags (bool) | 32 | `vocabulary.rs:334-367` | italic/bold per syntax role, `terminal.cursor-blink`, `terminal.bold-as-bright` |
| Terminal cursor style | 1 | `vocabulary.rs:370-373` | `block`, `underline` or `bar` |
| Terminal line height | 1 | `vocabulary.rs:377-380` | 1.0–2.0 |
| Motion durations | 5 | `vocabulary.rs:138-144` | integer ms, 0–10 000 (`LONGEST`, `:156`) |
| Motion easings | 4 | `vocabulary.rs:147-152` | cubic Bézier, x in 0–1 and y in −1 to 2 (`schema.rs:126-141`) |
| Geometry tokens | **31** (writing 35 custom properties; the 2 font families come from `fonts`) | `geometry.rs:83-280` (`STEPS`) | see 6.2 |
| Fonts | 3 slots (UI, mono, terminal) | `fonts.rs` (`SLOTS`) | woff2 files in the theme folder only (`schema.rs:67, 276-290`) |
| Icon set (rails) | 11 views | `vocabulary.rs:293-296` (`RAIL_VIEWS`) | path data, no SVG text |
| Options | toggles and choices; **no free-number options** | `schema.rs:177-181` | each value is a token layer |
| Tint recipe | `on`, `tokens` (from `TINTABLE`), `hues` | `schema.rs:201-223`, `vocabulary.rs:190-229` | |

#### 6.2 Geometry tokens and their bounds (`geometry.rs:83-280`)

Every value is a whole number except the line heights. A value out of bounds **refuses the whole theme** (`geometry.rs:320-346`, `number()`).

| Token | Bounds | Classic (comfortable / compact) | Unit | Uses in CSS (`var(--…)`) |
|---|---|---|---|---|
| space.0 | 0–0 (fixed) | 0 | text px→rem | – |
| space.1 | 0–4 | 2 / 2 | rem | |
| space.2 | 1–8 | 4 / 3 | rem | |
| space.3 | 2–10 | 6 / 4 | rem | |
| space.4 | 4–12 | 8 / 6 | rem | |
| space.5 | 6–16 | 12 / 8 | rem | |
| space.6 | 8–24 | 16 / 12 | rem | |
| space.7 | 12–32 | 24 / 16 | rem | |
| space.8 | 16–48 | 32 / 24 | rem | |
| font-size.2xs | 8–12 | 10 | rem | 1 |
| font-size.xs | 9–13 | 11 | rem | 74 |
| font-size.sm | 10–14 | 12 | rem | 37 |
| font-size.md | 11–15 | 13 | rem | 13 |
| font-size.base | 12–18 | 14 | rem | 12 |
| font-size.lg | 13–22 | 16 | rem | 3 |
| font-size.xl | 16–28 | 20 | rem | 3 |
| font-weight.regular | 300–500 | 400 | weight | 9 |
| font-weight.medium | 400–600 | 500 | weight | 3 |
| font-weight.strong | 500–800 | 600 | weight | 44 |
| line-height.tight | 1.0–1.4 | 1.2 | ratio | 2 |
| line-height.body | 1.2–1.8 | 1.4 / 1.3 | ratio | 5 |
| **radius.sm** | **0–8 px** | 3 | px (does not scale with text) | 24 |
| **radius.md** | **0–10 px** | 4 | px | 45 |
| **radius.lg** | **0–14 px** | 6 | px | 7 |
| **radius.xl** | **0–18 px** | 8 | px | 2 |
| radius.full | 0–9999 px | 9999 | px | 17 |
| size.control | 20–40 | 28 / 24 | rem | 25 |
| size.chrome | 16–32 | 21 / 18 | rem | 17 |
| focus-ring.width | 1–4 px | 2 | px | 16 |
| focus-ring.offset | 0–4 px | 2 | px | 8 |
| focus-ring.inset | −4 to 0 px | −2 | px | 4 |
| elevation.raised-y / -blur | 0–8 / 0–24 | 2 / 8 | rem | 1 (`--elevation-raised`) |
| elevation.overlay-y / -blur | 0–16 / 0–48 | 8 / 24 | rem | 1 (`--elevation-overlay`) |

- The Classic values are in `app/src/theme/geometry.css:20-84`. The engine writes them byte for byte (`geometry.rs:395-430`).
- The single cross-token check is `FITS` (`check.rs:659-670`): `font-size.base × line-height.body` must fit in `size.control`, and `font-size.md × line-height.body` must fit in `size.chrome`. Nothing checks radius against height.
- The built-in **Purlis** theme already rounds more than Classic: `radius.md 7`, `radius.lg 10`, `radius.xl 12`, `size.control 26` and `size.chrome 24` (`built-in/purlis/theme.json:212-216`, repeated at `:390-394` and `:569-573`). The engine therefore does take rounded corners, inside the caps.

#### 6.3 Surfaces with no token at all (hard-coded in CSS)

The figures are for `App.css` (8 683 lines), counted over declarations.

| What | Count | Evidence |
|---|---|---|
| **Tab shape**: `border-radius: 0` on every strip tab (projects, workspaces, chats) | 1 rule, 3 strips | `App.css:441`, in `.projects/.workspaces-strip/.tabs [role="tab"]` (`:420-451`). The comment says "flat, because the cell around it is the shape" (`:418`). The cell (`:370-380`) has no radius either. |
| Tab `×` closer and settings gear: `border-radius: 0` | 2 | `App.css:517` (`.tab .closer, .project .closer`) and `App.css:554` (`.project .gear, .workspace .gear`) |
| **List/tree row primitive**: `border-radius: 0` | 1 rule, 30 `ui-row` and 13 `ui-tree-row` call sites | `primitives/collections.css:210` (`.ui-row`) |
| Window notices list and Welcome recents: `border-radius: 0` | 2 | `App.css:4902`, `App.css:5162` |
| Dots: `border-radius: 50%` literal | 2 | `App.css:4485`, `App.css:7468` |
| Tab selected style: background only, `layer.selected` | no "tab style" token (underline, edge, pill) | `App.css:409-416` |
| Strip heights | title bar `min-height: 2.1rem` is a literal | `App.css:5802`. The workspace strip uses `--size-control` (`:268`, `:357`) and the chat bar uses `--size-chrome` (`:763`). The three strip heights are 2.1rem, 28px and 21px, and only two of them can be themed. |
| Activity-bar tab height 2.5rem and icon 1.25rem | literal | `App.css:2763`, `:2773` |
| **Focus ring written as a literal `2px` / `-2px`** although tokens exist | **21** `outline: 2px` in App.css (22 across all CSS), against 16 token uses in total. Also 19 literal `outline-offset: ±2px`, 3 at `1px` and 1 at `-1px`. | e.g. `App.css:157, 501, 538, 578, 864, 1055, 1103, 1394, 1853, 2028, 2345, 2534, 2782, 4367, 4464, 4643, 5905, 7010, 7967, 8123, 8426`. The status bar does it right (`App.css:5341-5342`). |
| Border widths | **85** `1px solid` in App.css; no border-width token exists | — |
| Opacity fades used as text ranks | about 36 literal fractions (0.45 ×8, 0.7 ×9, 0.92 ×6, 0.8 ×3 …) | e.g. `.honest { opacity: 0.8 }` at `App.css:4427-4431`, used at 82 sites in 22 files |
| Literal widths and heights (rem/px/em) | **155** declarations: dialog widths, icon sizes (1em ×7, 0.8rem ×5, 0.75rem, 0.45rem …), `max-width: 60rem` ×11 | — |
| Letter-spacing | 7 literals (0.06em); `primitives/text.css` says "no token" | `text.css` (`.ui-text-label`) |
| Count-pill minimum widths | `1rem` (`.activity-count`, `App.css:2807`) and `1.3rem` (`.needs-you-number`, `:4663`) | — |

**What is clean.** App.css has **0** hex/rgb/hsl colour literals in declarations. Every colour goes through a token, with 4 `color-mix(currentcolor…)`. It also has 0 literal font sizes (117 token, 4 `inherit`), 0 literal font weights (45 token) and 0 literal padding/margin/gap (all `--space-*`). Of its 85 `border-radius` declarations, 77 are tokens, 5 are `0` and 2 are `50%`. A literal guard (`literals.test.ts`, named in `theme.ts`, not fetched) covers colour.

**A misleading name a theme author will hit: `tab.active`.** No tab uses it. It is the **menu row highlight** (`primitives/controls.css:552`), the **primary-button hover** (`:67`), the **pressed-toggle hover** (`:109`) and the tasks-menu places (`App.css:2237`). The selected strip tab is `layer.selected` (`App.css:416`). The skill names `tab.active` as part of "most of a look" and says to mix it from the brand colour (`SKILL.md:62-63, 69-70`). Doing so recolours menus and button hovers, not tabs.

#### 6.4 Why a theme author is told corners can't be rounded

No source says this outright. Grepping `rounded`, `corner` and `radius` over the skill, `check.rs`, `floors.rs` and `hard.rs` finds nothing about corners. The belief follows from five things in the code:

1. **The skill never mentions geometry.** `SKILL.md:60-64`: *"Most of a look is `surface.*`, `text.*`, `border.*`, `accent.base`, `accent.surface`, `focus.ring`, `tab.active`, `list.selected`, `list.selected-edge` and the `terminal.*` colours."* Its done-condition, `SKILL.md:98`, is *"Done when the file is JSON with every value hex, a palette name, or a token's own type."* None of its starting points (brand colour, image, Figma frame, imported theme) produces a size or radius (`SKILL.md:68-89`). The full format is deferred to `purlis docs show themes` (`SKILL.md:60`), which was not fetched.
2. **The engine's own report header still says geometry is future work.** `check.rs:678-681`: *"This shape is today's (format 1) and will change for format 2 (spec #2313): typed values beyond colour and motion (geometry, fonts, icons)…"*. This was written before #2326 landed geometry. An agent reading the check's types concludes radius is not a theme value.
3. **The surfaces a person looks at most are flat in CSS whatever the token says.** These are the tabs (`App.css:441`), the tab closer and gear (`:517`, `:554`) and every list and tree row (`collections.css:210`). A theme that sets `radius.md: 10` changes buttons, fields and overlays, but no tab and no row. Here "can't round" is true in practice.
4. **The caps are low.** `radius.md` is 0–10px and `radius.lg` is 0–14px (`geometry.rs:193-196`), and corners are px, not scaled with text size (`geometry.css:18`). A pill-shaped button needs `radius.full`, which only counts and toggle tracks use (17 sites). No token lets a theme make buttons pills.
5. **The importer and the format-1 path carry no geometry.** `import.rs` has no geometry references, so a VS Code, base16, iTerm2 or Ghostty import brings colours only. A format-1 file that names geometry is only told where it goes (`vocabulary.rs:313-318`, `names_geometry`).

**For the program:** the engine supports radius (format 2, `geometry.rs`). The gaps are (a) the skill text, (b) the stale `check.rs` comment, (c) the zero-radius rules on tabs and rows, (d) no tab-shape/tab-indicator tokens, and (e) no button-shape token separate from field radius.

---

### 7. Plugin and extension seams for UI

#### What an approved extension can contribute today

The registry is described in `Extensions.tsx:52-80`: *"each capability it asks for first (ADR 0053), then its themes, panels, views and program."* Nothing is in force until the extension is approved (`ApproveExtension.tsx`).

| Contribution | Where it shows | Wire type / code |
|---|---|---|
| Colour themes (theme folders, id `<extension>/<theme>`) | Settings › Theme, whole window | `SKILL.md:35`; `Extensions.tsx:396-441` (`themeTroubleSaid`, `drawThemeFor`) |
| File icon themes | Explorer and trees | `theme/icons.ts:4-35`: path data only, colours must be `icon.*` tokens; complaints at `Extensions.tsx:364-395` |
| Rail icon set (through its theme's `icons`) | Activity bars, 11 view ids | `vocabulary.rs:293-296`, `theme/railIcons.ts` |
| **Panels**: one right-region view each | Right activity bar, a tab per panel | `PanelView` (`bindings.ts:7835-7850`). Blocks are a closed set: `list`, `note`, `chart` (answers only) and `facts` (`bindings.ts:7734-7743`). The mark is a closed set (`Panels.tsx:274`, `MARKS`, fallback `Circle`). Drawn by `PanelSection` and `PanelList`. |
| **Views** (tab content) | A view tab | `ExtensionView` (`bindings.ts:5759`), `Views.tsx:135-180`: the "panel vocabulary and nothing else: notes, facts, lists and charts" |
| Palette commands | ⌘K palette, prefixed with the extension name | `ExtensionCommand` (`bindings.ts:5680-5691`), `Views.tsx:181` |
| Row actions (with ask-first or delete confirm) | A row of its view/panel, or the palette | `RowAction` (`bindings.ts:9256-9266`), `ExtensionAction.tsx` (AlertDialog plus AnswerBar) |
| Status-bar badges | Status line, after purlis's own counts; dimmed when stale | `FactBadge` (`bindings.ts:5775`), `StatusLine.tsx:236-260`, `extensionFacts.ts` |
| Repo-table columns | Repos table | `FactColumn`/`FactCell` (`bindings.ts:5788-5803`) |
| Notes ("contributed nothing") | Status line "N extension notes" | `ExtensionFacts.notes`, `StatusLine.tsx:254-260` |

What an extension **cannot** contribute: CSS, markup, its own components, a dialog of its own, a Notice, a title-bar or strip item, a settings group, or geometry. All of it is data that purlis draws with its own primitives (ADR 0041 "parse-and-re-emit"). The runtime that produces the data lives in `app/src-tauri/src` (`panels.rs`, named at `Panels.tsx:62-64`) and `purlis_core::extension` and `::panel`. None of those were fetched.

**What is themeable for an extension surface.** Everything is drawn by purlis's CSS, so the colour and geometry tokens reach extension panels, views, badges and columns exactly as they reach purlis's own. No token or selector targets "contributed" surfaces separately: provenance is only a `data-panel-from` attribute and a ` · <name>` suffix (`PanelSection.tsx:37-66`). An extension can theme the whole window by shipping a theme, but cannot style its own panel apart from the rest.

**`app/src-tauri/plugin` is not a UI seam.** It is the harness plugin: `.claude-plugin/plugin.json` ("purlis's hooks, guard and skills for the chats the purlis app starts"), `hooks/hooks.json`, `opencode/purlis.ts` and 11 skills. It contributes no window UI.

---

# 8. Top 15 inconsistencies, ranked by how often a person sees them

"Always" means it is on screen in every window at every moment. Ranking is by exposure, not by how hard the fix is.

| # | Inconsistency | Seen | Evidence |
|---|---|---|---|
| 1 | **The title-bar row mixes four button styles.** Needs-you is ~15px and borderless (`App.css:4627`). Save is a 21px ghost `ui-button-sm`. Kill switch, About and Update are content-sized ~17px ad-hoc copies of one rule (`App.css:5868, 5850, 5402`). Text is xs next to sm. About is `--text-secondary` while Update is primary, muted or accent, and icon-only when quiet. The comment at App.css:5849 claims "one row, one shape". The title bar is xs (11px) while the status bar is sm (12px). | always | `TitleBar.tsx:174-195`, `About.tsx:98`, `Updates.tsx:315`, `KillSwitch.tsx:92` |
| 2 | **Tabs and list rows are square whatever the theme says.** `border-radius: 0` is set on every strip tab, on the closer and gear, and on the `.ui-row` primitive (30 `ui-row` and 13 `ui-tree-row` sites). There is no tab-shape or tab-indicator token, so a theme's `radius.*` never reaches the surfaces seen most. | always | `App.css:441, 517, 554`; `primitives/collections.css:210` |
| 3 | **Selection and strip height are drawn three ways each.** Selection is a background fill on the strips (`layer.selected`, `App.css:409-416`), an inset 2px accent edge on the activity bars (`:2793-2799`), and an accent surface on pressed and segmented controls. Strip heights come from three sources: `2.1rem` literal (title, `:5802`), `--size-control` (workspaces) and `--size-chrome` (chats). The activity tab is a `2.5rem` literal (`:2763`). | always | as cited |
| 4 | **There is no badge or count primitive; 7+ copies differ.** They differ in radius (full vs sm-with-border), text colour (primary vs secondary), weight, padding (`--space-2` vs `--space-3`) and min-width (`1rem` vs `1.3rem` literals). | always (tabs, activity bar, explorer, status bar, chats, settings) | `App.css:590, 2803, 3463, 3636, 4662, 5390, 7732, 8273` |
| 5 | **There is no icon-button component and no icon-size token.** Icon buttons come in five or more families: ghost-sm, `.pane-find`, `.pane-doing`, closer and gear at radius 0 with padding 0, and the pill `.tab-tasks` button inside a square tab. Icon sizes are 0.75, 0.8, 0.85, 1 and 1.25rem literals, plus `1em` ×7. | always | `App.css:316, 517, 554, 1374, 1994, 2048-2773, 2514` |
| 6 | **Group headings at one rank are drawn four ways.** They are `.sidebar-title` + `.ui-text-label` (both worn at once with identical values), `.inbox-chain` (not uppercase), `.panel-day` (muted), and `.chats-group`, which uppercases the workspace name. The Explorer's "of" qualifier (12px) is bigger than the 11px label it qualifies. The uppercase letter-spacing is written out 4 times. | always (side panels) | `App.css:2949, 3532-3568, 5996`; `Explorer.tsx:1242-1259`; `ChatsSection.tsx:1002`; `text.css:59` |
| 7 | **The Inbox ranks the status above the chat's name.** The group heading (the chat name) is `h3.inbox-chain`: 11px, secondary. The ask line under it is 13px, strong, primary. "Waiting on your reply" is the `question` source's sentence, so a generic status outshouts the name by two sizes and a colour step. `InboxElsewhere` does the reverse. The Inbox's own scope switch is 10px (`2xs`), under the 11px floor, with muted counts. | every Inbox or needs-you open | `Inbox.tsx:427, 639, 820-833`; `InboxElsewhere.tsx:114`; `App.css:2925, 2949, 7835` |
| 8 | **Notice buttons and surfaces differ.** Every Notice (136 sites in 59 files) pairs bordered fixes with a borderless ghost "Dismiss". A long fix label such as "Update workspace layout" wraps inside `.ui-button-row` and grows taller than Dismiss. No Notice has a primary button. Pane notices are always amber-edged, even for news. Notice text is xs in a pane and sm in the Inbox, and its sm buttons are louder than its xs sentence. | several times a day | `Notice.tsx:185-265`; `InboxAlerts.tsx:199`; `controls.css:139-160`; `App.css:2311, 4223` |
| 9 | **There are two tooltip systems.** 219 native `title=` attributes in 85 files sit against 5 Radix tooltips. They give two looks and three delays: the OS's, 300ms and 600ms. A theme cannot reach native tooltips. Radix sites also vary in arrow and `sideOffset` (0/2/4/6/8). The status-bar tooltip has no maximum width. | almost every hover | `overlays/tooltip.tsx`; `primitives/MenuRow.tsx:21, 94`; `StatusLine.tsx:302, 318` |
| 10 | **Controls are a size louder than the text around them in every view pane.** `.ui-button` and `.ui-input` are 14px in 12px view panes, and `.ui-button-sm` is 12px in 11px pane notices. On every settings row and dialog form (`SettingRow`, 136 refs) the value is louder than its 12px label and its 13px group heading. 7 heading rules set no weight, so they render at the UA's 700 instead of the 600 token. | every view tab, every settings row | `controls.css:49, 116, 178`; `App.css:6248, 7973, 8236`; `App.css:4289, 5225, 5475, 6270, 7971, 8176` |
| 11 | **Dialog footers come in mirrored layouts.** `AnswerBar` (32 sites) sits on the right with Cancel first. `SettingActions` (56 sites) sits on the left with the act first. Extensions, Updates and Save show both. Unclassed footer buttons (~30px, subtle border) sit beside `.ui-button` (28px, strong border): Push/Land use them (`ChangeActions.tsx:67,72`), and so do ~20 form dialogs. Doctor's Fix and Settings buttons have no look at all (**check on screen**). | every dialog and form | `AnswerBar.tsx:27`; `settings/components.tsx:983`; `App.css:4441, 8406`; `Doctor.tsx:393, 593` |
| 12 | **Dialog actions scroll away.** 48 of 50 dialogs share `.warning`, which has `max-height: 80vh; overflow: auto` on the whole box and no sticky footer. Only Ask {persona} (sticky, `App.css:4383`) and Brief (flex column) keep their buttons in reach. About 20 dialogs lose the action row with long content. **Start a chat** is one: the Harness and Persona radio lists have no cap and Start/Cancel come last. Others are Quit/Reopen (`ul.ending`), Doctor, Approve plane and extension, and the delete lists. The palette nests two 55vh lists in an 80vh box. | every chat start in a plane with many personas, every quit or relaunch, every approval | `App.css:4279-4287, 4383, 4421, 4808`; `StartChat.tsx:276-563` |
| 13 | **Dialog width follows the content.** The base `.warning` sets only `max-width: 34rem`. There are 7 other width rules and 2 height caps (80vh vs `100vh-2rem`), and window margins written as 2rem, 92vw and 4rem. Two dialogs have no placement rule at all and are probably misplaced (**check on screen**): `ForgeGitLabSignIn.tsx:96` and `ThemeDraftStrip.tsx:332`. | every dialog (the two unplaced: rare) | `App.css:4279, 4301-4326, 4421, 4782, 5443, 5470` |
| 14 | **Settings rows don't align.** The label has no column (`max-inline-size: 40%`, `App.css:8221-8229`), so every control starts at a different x. Reset is bordered while "Move to…" beside it is ghost. Reset appears only for set values, so Move to… jumps sideways. One page holds 4 button boxes: `.ui-button` 28px, bare `.ui-setting-reset` ~23px in a pane (13 sites), unclassed `SettingActions` ~30px, and the base tier. The `ui-setting-reset` class draws two different buttons depending on whether `ui-button` is also present. | every Settings visit | `settings/components.tsx:374-408`; `SettingsTab.tsx:963-971, 1182`; `App.css:8298-8364, 8406` |
| 15 | **Long collections are ungrouped.** Your hosts, Persona hosts, the project hosts, profiles and the Network granted list each draw every item as a bold heading or sentence with inline Confirm and Remove. There is no table, no unconfirmed/confirmed split, no bulk action and no count, and the action x-position follows each name's length. Row actions sit in three different places across settings lists. `.ui-table` exists (5 uses, stacks under 29rem) and the Dispatch grants table is the model to follow. | every Sandbox page, growing with use | `settings/Collection.tsx:171-249`; `settings/project.ts:274-298, 638-645`; `settings/GrantedList.tsx:160-209`; `primitives/collections.css:20-176` |

**Next tier.** These are seen less, or by fewer people:
- **Focus rings.** 21 rings are a literal `2px` / `-2px`, so a theme's `focus-ring.width` misses the tabs, the activity bar and the title bar. Every keyboard user sees this constantly.
- **Two segmented controls.** `.ui-levels` is used; the `.ui-segmented` primitive has 0 call sites. This shows in the Settings level switcher and the Chats/Inbox scope switch.
- **Opacity as a text rank.** `.honest` at 0.8 has 82 sites, and dialogs use 0.55 and 0.7 instead of the text tokens.
- **The same question at two sizes.** A panel answer uses 17px `.panel-view` buttons and the dialog version 28px; inline task-end uses 21px with no primary, while the TaskEnd dialog has primary and danger.
- **Menu widths.** Widths and offsets vary per menu, and `.more-menu` has no maximum.
- **Welcome buttons.** These are the only `radius-lg` buttons.
- **Disabled and cursor drift.** Disabled is 0.45 in one place and 0.5 in another; the cursor is `pointer` in some places and `default` in others.
- **Dead CSS.** `.ui-segmented`, `button.bare` and `.ui-text-title` have no call sites.

**Author-facing, not ranked by view frequency.** These shape every theme an agent writes:
- **The skill covers colour only.** `SKILL.md:60-98` never mentions geometry, and its done-condition is "every value hex, a palette name, or a token's own type".
- **A stale comment says geometry is future work.** `check.rs:678-681` still calls geometry "format 2" future work, although `geometry.rs` ships 31 bounded geometry tokens.
- **`tab.active` never colours a tab.** It paints menu highlights and primary-button hovers (`controls.css:67, 109, 552`), but the skill tells authors to set it for the look of tabs.
- **Corners are capped and px-only.** The caps are `radius.md` 0–10px and `radius.lg` 0–14px, in px that don't scale with text, and there is no button-shape token.

Together with #2, this is why an author ends up believing corners can't be rounded. The engine itself accepts rounding: the built-in Purlis theme sets `radius.md` to 7.
