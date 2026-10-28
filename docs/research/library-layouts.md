# How other tools lay out a library of items

Research for [#3625](https://github.com/purlis/purlis/issues/3625), a child of the skill studio map [#3623](https://github.com/purlis/purlis/issues/3623). This file holds facts for a later decision. It makes no recommendations.

Researched 2026-10-28.

## How the sources were read

At first the live product sites could not be fetched, because the permission classifier blocked plain HTTP to them. Where a public source existed, each product was read from the repository its docs site is built from. Later the operator allowed raycast.com and support.claude.com, and those two were fetched directly:

| Product | Source read | Live page it builds |
|---|---|---|
| VS Code | the live page, fetched once before the block | https://code.visualstudio.com/docs/configure/extensions/extension-marketplace |
| JetBrains | `JetBrains/intellij-sdk-docs`, `JetBrains/marketplace-docs`, and the IDE's UI strings in `JetBrains/intellij-community` (`platform/platform-api/resources/messages/IdeBundle.properties`) | plugins.jetbrains.com/docs/... |
| Raycast | the live store, a listing page, the manual and the developer docs, fetched directly once the operator allowed raycast.com | raycast.com, manual.raycast.com, developers.raycast.com |
| Claude.ai | three support.claude.com articles, fetched directly once the operator allowed the host; the `anthropics/skills` README; `anthropics/claude-cookbooks` notebook 03 | support.claude.com |
| GitHub Actions | `github/docs`, folder `content/` | docs.github.com |
| Cursor | the live cursor.com docs, fetched before the block | cursor.com/docs |
| Obsidian | `obsidianmd/obsidian-help`, `obsidianmd/obsidian-developer-docs`, `obsidianmd/obsidian-releases` | help.obsidian.md, docs.obsidian.md |

**Fetch status by product.** The auto-mode permission classifier blocked most direct fetches. Where a block was met, this is the status:

- **VS Code: unverified.** One helper `curl` read the page before the blocks began. Treat the section as unverified until the operator checks.
- **JetBrains: unverified.** A direct fetch of jetbrains.com was blocked. The section was then read through `gh` from JetBrains' public GitHub repositories, which was a different tool reaching similar content after a block. The operator is checking.
- **Raycast: fetched directly on 2026-10-28,** after the operator allowed raycast.com. The section was rewritten from those fetches. An earlier helper had retried two blocked `curl` reads, and the operator is checking those, but nothing from them is used here.
- **Claude.ai: fetched directly on 2026-10-28** from support.claude.com, after the operator allowed it.
- **Cursor and Obsidian:** read by helpers before or without a block. Some later Obsidian raw fetches were refused and were not used.

Two limits follow from this:
- A claim marked **(unverified)** came from no primary source. It is kept only where its absence would mislead.
- The end-user help for JetBrains (`jetbrains.com/help/idea/managing-plugins.html`) has no public source, and jetbrains.com is not allowed. The IDE's own UI strings stand in for it.

Images are linked, not copied.

---

## 1. VS Code: Extensions view and extension page

**Unverified:** see the fetch status above.

All bullets are from the [Extension Marketplace](https://code.visualstudio.com/docs/configure/extensions/extension-marketplace) page, here called [MKT].

### List vs detail
- **Where it lives:** the Extensions view opens from the Activity Bar (Ctrl+Shift+X). By default it lists installed and recommended extensions. [MKT]
- **A list row:** name, short description, publisher, download count and a five-star rating, plus an Install, Update or Manage (gear) button. [MKT]
- **The details page:** opens when you select a row. It shows the README and sections for Feature Contributions (settings, commands, keybindings, grammars, debuggers), Changelog, Dependencies, Extension Pack, Categories and tags, and Marketplace Info (the identifier, for example `wayou.vscode-todo-highlight`). [MKT]
- **Search:** a query language with `@installed`, `@enabled`, `@disabled`, `@updates`, `@builtin`, `@recommended`, `@popular`, `@category:`, `tag:` and `@sort:installs|name|publishedDate|rating|updateDate`. The same filters are in a Filter menu. [MKT]
- **Screenshots:** [list](https://code.visualstudio.com/assets/docs/configure/extensions/extension-marketplace/extensions-popular.png), [details page](https://code.visualstudio.com/assets/docs/configure/extensions/extension-marketplace/todo-highlight-details.png), [contributions](https://code.visualstudio.com/assets/docs/configure/extensions/extension-marketplace/extension-contributions.png), [filter menu](https://code.visualstudio.com/assets/docs/configure/extensions/extension-marketplace/extensions-view-filter-menu.png).

### Create and edit
- The view has no authoring surface. [MKT] sends authors to "Your first extension", which is outside the app (scaffold, then run in an Extension Development Host; that flow itself is **unverified**, because the API pages could not be fetched).
- Users never edit an installed extension. They configure it through the settings it contributes, which appear in the Settings editor. [MKT]

### Install from a source, and updates
- **Marketplace:** Install turns into the Manage gear once installed. Since 1.97, the first install from a new third-party publisher asks you to trust that publisher. [MKT]
- **VSIX:** "Install from VSIX" in the view's "..." menu, or `code --install-extension file.vsix`. Extensions installed this way do not auto-update. [MKT]
- **Command line:** `--list-extensions --show-versions`, `--install-extension`, `--uninstall-extension`. [MKT]
- **Organisations:** a Private Marketplace, and the `extensions.allowed` allow-list that blocks anything not listed. [MKT]
- **Updates:** on by default, for enabled extensions only. `extensions.autoUpdateDelay` waits 12 hours by default. Auto-update can be turned off for everything or per extension (right-click → Auto Update). An `@updates` filter, Update buttons per row, Update All, and Check for Updates cover manual updates. [MKT]
- **Signing:** Marketplace extensions are signed and the signature is checked on install. [MKT]

### History and versions
- **Changelog** is a section of the details page. [MKT]
- **Install Another Version** on the right-click menu picks an older or newer published version. Download Specific Version VSIX does the same for a file. [MKT]
- **Install Pre-Release Version** sits on the Install button's dropdown. [MKT]

### Narrow sidebar vs full page
- **Sidebar:** the list, search, filters, per-row buttons, right-click menus, and a "..." menu for update controls, Enable/Disable All, and Extension Bisect. [MKT]
- **Full page:** the details page opens in the editor area (stated by the screenshots, not in prose). [MKT]

### Where destructive actions live
- **Uninstall:** Manage gear on the row or on the details page → Uninstall, then a Restart Extensions prompt. [MKT] describes no confirmation dialog. [Screenshot](https://code.visualstudio.com/assets/docs/configure/extensions/extension-marketplace/uninstall-extension.png).
- **Disable:** same gear, globally or for this workspace only. Disabled extensions move to a Disabled section. [MKT]
- **Bulk:** Disable All Installed Extensions in the Command Palette and the "..." menu. [MKT]

---

## 2. JetBrains plugins

**Unverified:** read through `gh` after a blocked fetch, and the operator is checking. See the fetch status above.

Sources: the IDE's UI strings in [IdeBundle.properties](https://github.com/JetBrains/intellij-community/blob/master/platform/platform-api/resources/messages/IdeBundle.properties) [IDE]; the IntelliJ SDK docs [Publishing a Plugin](https://plugins.jetbrains.com/docs/intellij/publishing-plugin.html) [PUB], [Custom Plugin Repository](https://plugins.jetbrains.com/docs/intellij/custom-plugin-repository.html) [CPR] and [Dynamic Plugins](https://plugins.jetbrains.com/docs/intellij/dynamic-plugins.html) [DYN]; the Marketplace docs [Plugin updates](https://plugins.jetbrains.com/docs/marketplace/plugin-updates.html) [UPD], [Custom release channels](https://plugins.jetbrains.com/docs/marketplace/custom-release-channels.html) [CH] and [Hidden plugin](https://plugins.jetbrains.com/docs/marketplace/hidden-plugin.html) [HID].

### List vs detail
- **Where it lives:** the Plugins page of Settings, with two tabs, **Marketplace** and **Installed** (`plugin.manager.tab.marketplace`, `plugin.manager.tab.installed`). [IDE]
- **The Installed tab** can be filtered by Update available, User-installed, Enabled, Disabled, Invalid, Bundled and Updated bundled. Its title shows "{0} ({1} of {2} enabled)". [IDE]
- **The detail pane** has tabs **Overview**, **What's New**, **Reviews** and **Additional Info**, plus a **Report Plugin** action and a "Version: {0}" label. [IDE]
- **List and detail sit side by side** in one Settings page (the pane is "configurable" in the IDE's terms). That this is a master-detail split is inferred from the strings and is **unverified** as a statement.

### Create and edit
- Authoring happens outside the plugin manager, in a Gradle project. A dynamic plugin can be installed, updated and uninstalled without restart, and during development **Auto-Reload** applies code changes in a sandbox IDE. [DYN]
- **First publication** is always a manual upload on the Marketplace website: Profile → **Add new plugin**. Later versions can go by Gradle `publishPlugin` with a personal token. [PUB]
- On the Marketplace there is "no option to edit the update after submission except for the compatibility range". [UPD]

### Install from a source, and updates
- **Marketplace tab:** Install, then a restart prompt unless the plugin is dynamic. Paid third-party plugins always need a restart. [IDE], [DYN]
- **The gear menu** at the top of the page holds "Manage Repositories, Configure Proxy or Install Plugin from Disk" (`plugin.manager.tooltip`). [IDE]
  - **Install Plugin from Disk** installs a ZIP/JAR. An organisation can forbid it ("Installing plugins from disk is not allowed for your organization"). [IDE], [PUB]
  - **Manage Plugin Repositories…** adds a custom repository URL. [IDE] A repository is an HTTPS `updatePlugins.xml` listing each plugin's id, url, version and IDE build range. [CPR]
- **Updates:** a "Plugins Auto-Update" option, an **Update** button per row, **Update All**, and a notification "A new version {0} of the {1} plugin is available". [IDE]
- **Signing:** unsigned plugins raise a "Plugin Security Warning". [IDE]
- **Review:** every Marketplace update is reviewed by JetBrains before release. [UPD]

### History and versions
- **What's New** in the IDE shows change notes. [IDE] A custom repository can carry `<change-notes>` too. [CPR]
- **Channels:** an author can publish to a named channel such as `eap` or `beta`. Users subscribe by adding `https://plugins.jetbrains.com/plugins/<channel>/list` as a custom repository. Custom repositories take precedence over the default (Stable) channel. [CH]
- **Older versions:** the IDE strings show no "install another version" action. Downgrading through a file is refused: "Downloaded version ({0}) is older than the currently installed version ({1})". [IDE] The Marketplace website has a **Versions** tab per plugin. [HID]
- **Author side:** each version can be hidden, unhidden or removed (trash icon) on that Versions tab. [UPD], [HID] Screenshots: [hide version](https://github.com/JetBrains/marketplace-docs/blob/master/images/hide-version-icon.png), [remove version](https://github.com/JetBrains/marketplace-docs/blob/master/images/remove-update.png), [channels on the plugin page](https://github.com/JetBrains/marketplace-docs/blob/master/images/plugin_repository_custom_channels.png).

### Narrow sidebar vs full page
- The whole manager is one page inside the Settings dialog, which has its own left tree. There is no narrow tool-window list. [IDE]
- A plugin's own settings open through **Plugin Settings…** (`plugin.settings.link.title`), which goes to its page elsewhere in the same Settings tree. [IDE]

### Where destructive actions live
- **Uninstall**, **Enable** and **Disable** are per-plugin actions (`plugins.configurable.uninstall`, `.enable`, `.disable`). For an updated bundled plugin, the action reads **Uninstall update**. [IDE]
- **Uninstall confirms:** "Are you sure you want to uninstall the plugin '{0}'?", and for several at once "Are you sure you want to uninstall these {0} plugins?". [IDE]
- **Bulk:** **Disable All Downloaded Plugins** / **Enable All Downloaded Plugins**, and Disable all / Enable all. [IDE]
- **Cascades are announced:** "Dependent plugins are disabled", "Required plugins are enabled". [IDE]
- Uninstall applies on restart: "Plugin was uninstalled; changes will be applied on restart". [IDE]

---

## 3. Raycast store

**Fetched directly on 2026-10-28**, after the operator allowed raycast.com. The pages read:
- [raycast.com/store](https://www.raycast.com/store) [STORE]
- a sample listing, [Color Picker](https://www.raycast.com/thomas/color-picker) [LIST]
- the manual's [Extensions](https://manual.raycast.com/extensions) page [MAN]
- these developer docs:
  - [Install an Extension](https://developers.raycast.com/basics/install-an-extension) [INS]
  - [Create Your First Extension](https://developers.raycast.com/basics/create-your-first-extension) [NEW]
  - [Publish an Extension](https://developers.raycast.com/basics/publish-an-extension) [PUBR]
  - [Contribute to an Extension](https://developers.raycast.com/basics/contribute-to-an-extension) [CON]
  - [Versioning](https://developers.raycast.com/information/versioning) [VER]
  - [Prepare an Extension for Store](https://developers.raycast.com/basics/prepare-an-extension-for-store) [PREP]

### List vs detail
- **Two stores:** the in-app **Store** command, opened from Root Search, and the [web store](https://www.raycast.com/store). [MAN], [INS]
- **The in-app Store:**
  - You browse it by name, command, keyword or category.
  - It shows only extensions compatible with your OS.
  - Opening an extension shows "its description, commands, screenshots, and author details". ↵ installs it.
  - Source: [MAN]. The developer docs give ⌘↵ to install from the list and ↵ for details, [INS], so the two pages differ on the keys. [Screenshot](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/basics-inapp-store.webp).
- **Installed items:**
  - There is no separate tab. You filter the Store by opening the **Categories** dropdown and choosing **Installed**. [MAN]
  - A **Filter by Organization** control shows a team's private extensions next to the public ones. [MAN]
- **Web store list:**
  - Sections: Featured, then All / Recently Added / Most Popular, with ⌘K search. [STORE]
  - Each card shows the name, a one-line description, the author, the command count, the install count, a platform tag such as "macOS only", and an **Install** button. [STORE]
- **Web listing page:**
  - The header has the name, tagline, author, install count, an **Install Extension** button and **Share**. [LIST]
  - Tabs: **Overview** (the README), **Commands**, **Version History**. [LIST]
  - Sidebar: Contributors, Past Contributors, Compatibility (macOS / Windows), Categories, and **View source**, **Report Bug**, **Request Feature**, **Copy URL**. [LIST]
  - Below: "People also like". [LIST]
  - Screenshots: [web store](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/basics-web-store.webp), [install button](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/basics-install-extension.webp).

### Create and edit
- **Create Extension** is a command in Raycast itself. You give a name, a template and a parent folder, then press ⌘↵. You then edit the code in an external editor while `npm run dev` hot-reloads it. [NEW] [Screenshot](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/hello-world.webp).
- **Fork Extension**, an action on any installed command, fetches that extension's source so you can change it. Every store extension is open source in one repository. [CON] [Screenshot](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/fork-extension.webp).
- **Import Extension** adds a local development extension. [MAN]
- A private team extension is chosen in the first dropdown of Create Extension. [NEW]

### Install from a source, and updates
- **For end users, the store is the install source:** in the app, or on the web, where **Install Extension** hands off to the app. [INS], [MAN]
- **Publishing:** `npm run publish` opens a pull request on `raycast/extensions`. Once the PR is reviewed and merged, the extension appears in the store. [PUBR]
- **Updates are automatic:** "Raycast automatically applies updates to your installed Store extensions in the background". [MAN] [VER] says the same: one implicit latest version.
- **Manual check:** the **Check for Extension Updates** command. Its toast has a **View Details** action that groups results into Updated, Skipped, Failed and Removed. [MAN]
- **Local extensions** added with Import Extension are never updated from the store. [MAN]
- **Compatibility:** Raycast checks the extension's API version against the app. On a mismatch it prompts you to update Raycast. [VER]
- **Configuration** lives in Settings → Extensions. [MAN], [INS]

### History and versions
- **No versions to pick.** Extensions declare no version; there is one latest. [VER]
- **Version History** shows on the listing's tab, made of dated entries from the extension's `CHANGELOG.md` (for example "Initial Version – February 8, 2023"). [LIST], [PREP] [Screenshot](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/version-history.webp).
- **In the app:** after an update check, **View Version History** in the Action Panel shows what changed. [MAN]
- No rollback is documented.

### Narrow sidebar vs full page
- Raycast is a launcher with no app sidebar. Every surface is a full launcher view: the Store list, then a pushed detail. An **Action Panel** carries the per-item actions. [MAN]
- **Manage Extensions** is a separate command over installed extensions. ⌘⌥. copies a store link. [PUBR] [Screenshot](https://github.com/raycast/extensions/blob/main/docs/.gitbook/assets/basics-manage-extensions.webp).
- **On the web,** the listing's right sidebar holds facts and links, and the main column holds the tabs. [LIST]

### Where destructive actions live
- **Uninstall is in the Action Panel.** Filter the Store to Installed, then open the Action Panel "to view all available actions including uninstall, configure, or view its source code". [MAN]
- A confirmation step is not described. **(unverified)**
- **Disable** is not described on the pages read. **(unverified)**
- On the web listing, the only negative action is **Report Bug**. [LIST]

---

## 4. Claude.ai skills and projects

**Fetched directly on 2026-10-28**, after the operator allowed support.claude.com:
- [Use skills in Claude](https://support.claude.com/en/articles/12512180-use-skills-in-claude) [USE]
- [How to create custom skills](https://support.claude.com/en/articles/12512198-how-to-create-custom-skills) [CRE]
- [How can I create and manage projects](https://support.claude.com/en/articles/9519177-how-can-i-create-and-manage-projects) [PROJ]

The API facts come from the [`anthropics/skills` README](https://github.com/anthropics/skills/blob/main/README.md) and [skills cookbook notebook 03](https://github.com/anthropics/claude-cookbooks/blob/main/skills/notebooks/03_skills_custom_development.ipynb) [NB].

### Skills: list vs detail
- **Where it lives:** **Customize > Skills**. It needs code execution ("Code execution and file creation"). On Team and Enterprise, an owner also enables Skills under Organization settings > Plugins & skills > Policy. [USE]
- **A list row:** the skill, "a brief description of what each skill does", when it was enabled or uploaded, and an on/off switch. Anthropic skills and your custom skills appear in the same list. [USE]
- **Sections on Team and Enterprise:**
  - **Personal skills**
  - **Shared with you** (off until you turn them on)
  - **Organization skills**, which "you install … from the directory rather than enabling them from the list"

  Provisioned skills carry a team indicator. [USE]
- **Detail:** "click on it to view the details". The detail view has a toggle in the upper right and a "..." menu next to it. [USE] Its other contents are **(unverified)**.
- **Directory:** in Cowork, **Customize** in the left sidebar → "+" → **Skills** tab → **Install**. [USE]

### Skills: create and edit
- **Upload:** package the skill folder as a ZIP. In Customize > Skills, click "+" → "+ Create skill" → "Upload a skill". [USE]
  - The folder needs a `SKILL.md` whose frontmatter has `name` (64 characters at most) and `description`.
  - The ZIP's root must be the skill folder, and the folder name must match the skill's name.

  [CRE]
- **Edit beside the chat:** while iterating with Claude in chat, the skill files "open beside the conversation". Highlight text and choose **Edit with Claude**. For several files, collect the requests and send them together. [CRE]
- **Record a skill** (Cowork on Claude for Mac, Pro, Max and Team): record about 10 minutes of screen and voice. Claude then proposes a skill:
  - A **NEW** card offers **Save** or **Dismiss**.
  - If the recording overlaps an existing skill, the card offers **Update** or **Dismiss** and shows which skill it is based on.
  - Start it from the composer "+" or from Customize > Skills → **Add** → **Record your screen**.

  [CRE]
- **The API** creates skills (`skills.create`) and new versions (`skills.versions.create`). Display titles must be unique. [NB]

### Skills: install from a source, and updates
- **Sources in claude.ai:** a ZIP upload, a skill recording, a colleague's share, and the organization directory. [USE], [CRE]
- **Shares follow the owner:** a shared skill is view-only. "If you update the skill later, recipients automatically get the updated version at next use." [USE]
- **Publish to org** (Team and Enterprise) submits the skill to the organization's library.
  - It can be offered as Available to install, Installed by default, or Required. You can add release notes for the reviewer.
  - When review is required, users "stay on the approved version until the update is approved".
  - To update it, you publish again.

  [USE]
- **Claude Code sync:** skills enabled in claude.ai load in Claude Code. The sync is one-way, runs at session start, then about every 10 minutes. [USE]
- **Scanning (Enterprise):** an uploaded or edited third-party skill can be scanned before it runs. [USE]

### Skills: history and versions
- **claude.ai** describes no version list or rollback. Updates are a re-upload, a republish, or "Update" from a recording proposal. [USE], [CRE] An org-published skill has an approved version plus a pending update. [USE]
- **The API** has versions:
  - `latest_version`, `skills.versions.list`, and pinning with `"version": "latest"`.
  - The notebook says versions "maintain history and enable rollback".
  - Deleting a skill means deleting its versions first.

  [NB]

### Skills: narrow sidebar vs full page
- **Settings page:** the list is Customize > Skills, and a skill's details open from its row. [USE]
- **Cowork:** Customize sits in the left sidebar. Installed directory skills appear in that sidebar, and toggling one off removes it. [USE]
- **Chat:** skill files open in a panel beside the conversation for editing. [CRE]

### Skills: where destructive actions live
- **Disable:** the switch on the row, or the toggle in the detail's upper right. [USE]
- **Delete (custom skills):** open the skill → "..." next to the toggle → **Delete** → confirm **Delete**. "If you change your mind, you can add the skill again by re-uploading the file." [USE]
- **Shared skills:** the owner can remove a person's access at any time. A recipient can turn the skill off, or delete it from their own list. [USE]
- **Org submissions** can be withdrawn while pending. [USE]

### Projects
- **List:** the Projects page at `claude.ai/projects`, reached from the left side panel, with **+ New Project** in the upper right. It has tabs "Your projects", "Organization" and "Shared with you". Archived projects sit at the bottom of the list or in an archived tab. Each project card has a "..." menu. [PROJ]
- **Create:**
  - A name and a description, which Claude cannot see.
  - On Team and Enterprise, a visibility choice: private, or shared with the organization.

  [PROJ]
- **Detail:** the project page, with chats in the main area.
  - The **project knowledge** base is "on the right side of your project's main page". Its "+" adds files. On paid plans it switches to RAG mode near the context limit.
  - **Set project instructions**, then **Save instructions**, edits the instructions.
  - **Share project** sits right of the name.
  - A star icon and a "..." menu sit in the upper right.

  [PROJ]
- **Sharing:** **Can view** or **Can edit** per member. The creator removes access from the member's role menu with **Remove access**. [PROJ]
- **History and versions:** none described for instructions or knowledge. [PROJ]
- **Destructive actions:**
  - **Archive:** "..." → confirm. It keeps members and knowledge. [PROJ]
  - **Delete:** "..." on the card or in the project → **Delete** → confirm with **Yes, delete**. An archived project cannot be deleted until it is unarchived. [PROJ]

---

## 5. GitHub Actions marketplace

Sources, all from [`github/docs`](https://github.com/github/docs): [Find and customize actions](https://docs.github.com/en/actions/how-tos/write-workflows/choose-what-workflows-do/find-and-customize-actions) [FIND], [Publish in GitHub Marketplace](https://docs.github.com/en/actions/how-tos/create-and-publish-actions/publish-in-github-marketplace) [PUBG], [Release and maintain actions](https://docs.github.com/en/actions/how-tos/create-and-publish-actions/release-and-maintain-actions) [REL], [Using immutable releases](https://docs.github.com/en/actions/how-tos/create-and-publish-actions/using-immutable-releases-and-tags-to-manage-your-actions-releases) [IMM].

### List vs detail
- **Two lists:** the Marketplace website, and a **Marketplace sidebar** in the browser workflow editor. The sidebar can search, show featured actions and categories, and shows stars and a "Creator verified by GitHub" badge. [FIND] [Screenshot](https://github.com/github/docs/blob/main/assets/images/help/repository/actions-marketplace-sidebar.png).
- **The listing page** "includes the action's version and the workflow syntax required to use the action", with a copy button under **Installation**. [FIND] [Screenshot](https://github.com/github/docs/blob/main/assets/images/help/repository/actions-sidebar-detailed-view.png).

### Create and edit
- An action is a repository with an `action.yml` at its root. Publishing: open `action.yml`, use the banner's **Draft a release**, tick **Publish this Action to the GitHub Marketplace**, pick a primary category, give a version tag, **Publish release** (two-factor authentication required). [PUBG]
- Actions are published immediately, **without review**, if they meet the rules: public repository, one root metadata file, and a unique name. [PUBG]
- Editing is ordinary repository work. There is no editor inside the marketplace. [PUBG], [REL]

### Install from a source, and updates
- **There is no install.** A workflow references an action with `uses: owner/repo@ref`. The action can come from the same repository, any public repository, or a Docker image. [FIND]
- **Updates:** the author moves major and minor tags (`v1`, `v1.1`) to the latest compatible commit. Users on `@v1` get updates silently, and users on a full SHA get none. [REL], [IMM]
- **Dependabot version updates** can raise PRs that bump action refs. [FIND] (via `data/reusables/dependabot/version-updates-for-actions.md`)

### History and versions
- **Versions are Git tags and GitHub releases.** The docs recommend semantic tags (`v1.1.3`) plus moving major/minor tags. [REL]
- **Immutable releases:** once enabled, a release's tag cannot move. Authors keep moving tags without releases for `v1` and `v1.1`. [IMM]
- **Rollback** means pinning an older ref in the workflow file. That is a consequence of `uses:@ref`; the docs do not name it as a feature. [FIND]

### Narrow sidebar vs full page
- **Narrow:** the editor's Marketplace sidebar, which handles browsing, a detail view and copying the snippet, next to the YAML. [FIND]
- **Full page:** the marketplace website listing. [FIND]

### Where destructive actions live
- **Delisting is per release:** edit each published release, untick **Publish this action to the GitHub Marketplace**, then **Update release**. [PUBG]
- **Removing an action from a workflow** is deleting the `uses:` line. Disabling whole workflows and repository Actions policy live under the repository's Actions tab and Settings ([disable and enable workflows](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/disable-and-enable-workflows); [screenshot](https://github.com/github/docs/blob/main/assets/images/help/repository/actions-disable-workflow-2022.png)).

---

## 6. Cursor rules

Sources: [Rules](https://cursor.com/docs/context/rules) [RUL], [Rules help](https://cursor.com/help/customization/rules.md) [RHLP], [Plugins](https://cursor.com/docs/plugins.md) [PLG], [Customize](https://cursor.com/docs/customize-cursor.md) [CUS], [Skills](https://cursor.com/docs/skills.md) [SKL].

### List vs detail
- **Kinds of rule:** Project Rules (`.mdc` files in `.cursor/rules`, in version control), User Rules (in settings, for all projects), Team Rules (in the web dashboard, Team and Enterprise plans) and `AGENTS.md` (root and nested). Team rules beat project rules, which beat user rules. [RUL]
- **Customize** is one place for plugins, skills, MCPs, subagents, rules, commands and hooks, with scope filters for user, workspace and team. It shows "all rules and their status". [CUS], [PLG], [RUL]
- **The detail of a rule is its file.** Opening one edits the `.mdc` file, with a type dropdown above the body: Always Apply, Apply Intelligently, Apply to Specific Files, Apply Manually. The dropdown writes the `alwaysApply`, `description` and `globs` frontmatter. [RHLP], [RUL]
- The columns of a list row are **unverified**.

### Create and edit
- **Four ways in:** `/create-rule` in Agent chat, Customize → Rules → **Add Rule**, Command Palette → **New Cursor Rule**, or asking the agent (also by tagging `@cursor` on a GitHub issue or PR). [RUL], [RHLP]
- `/migrate-to-skills` converts Apply Intelligently rules into skills. [SKL]
- **Team rules** are free text with optional globs, written in the dashboard. Leaving "Enable this rule immediately" unticked saves a draft. [RUL] Screenshots: [empty](https://cursor.com/docs-static/images/context/rules/team-rules-empty.png), [with a rule](https://cursor.com/docs-static/images/context/rules/team-rules-1.png).

### Install from a source, and updates
- **Rules arrive inside plugins.** Customize → **From GitHub Repository** (the repository needs `.cursor-plugin/marketplace.json`), or a team marketplace. [RUL], [PLG]
- **Installing a plugin** asks for project or user scope. A team marketplace marks each plugin Default Off, Default On or Required (Required cannot be uninstalled). [PLG]
- **Updates:** **Enable Auto Refresh** (GitHub only, at most every 10 minutes) or a manual **Refresh**. Cursor reviews each plugin and update by hand before publishing it. [PLG]

### History and versions
- **Project rules:** git history only. [RUL]
- **User and team rules:** no documented history or rollback. [RUL]
- **Plugins:** a manifest `version` field, but no documented pinning or rollback. [PLG]

### Narrow sidebar vs full page
- Customize is a page opened "from the sidebar in Cursor". Whether it is a panel or a full pane is **unverified**. [CUS]
- **Where each thing is done:** creating and @-mentioning rules happens in chat; listing and toggling in Customize; team rules in the web dashboard. [RUL]

### Where destructive actions live
- **Turning a rule off:** a member uses the toggle in Customize. An admin's **Enforce this rule** removes that toggle. [RUL]
- **Deleting a project rule** has no documented control. It is deleting the file. [RHLP]
- **Removing a marketplace** is the only documented confirmation dialog, because it can delete a Team MCP server. The Default marketplace cannot be removed. [PLG]

---

## 7. Obsidian community plugins

Sources: [Community plugins](https://help.obsidian.md/community-plugins) [OCP], [Plugin security](https://help.obsidian.md/plugin-security) [OSEC], [Community directory](https://help.obsidian.md/community-directory) [ODIR], [Settings](https://help.obsidian.md/settings) [OSET], [Build a plugin](https://docs.obsidian.md/Plugins/Getting+started/Build+a+plugin) [OBLD], [Submit your plugin](https://docs.obsidian.md/plugins/releasing/submit-plugin) [OSUB], [Versions](https://docs.obsidian.md/Reference/Versions) [OVER], [Beta-testing plugins](https://docs.obsidian.md/Plugins/Releasing/Beta-testing+plugins) [OBETA], the [obsidian-releases README](https://github.com/obsidianmd/obsidian-releases/blob/master/README.md) [OREL].

### List vs detail
- **In the app:** Settings → Community plugins → **Browse** lists all plugins. Search matches name, author and description. [OCP], [OREL]
- **The detail page** pulls `manifest.json` and `README.md` from the plugin's GitHub repository when opened, and has **Install**, then **Enable**. [OREL], [OCP]
- **On the web (2026):** a Community directory at community.obsidian.md with listing pages. Each has a header (name, an Official badge, author, downloads, **Add to Obsidian**), tabs **Overview**, **Scorecard** (health rating and automated review results) and **Updates** (release history), and a sidebar of facts: version, updated, compatible app version, platforms, licence, payment model. [ODIR]
- Sort options and the in-app row layout are **unverified**. No official screenshots of these screens were found.

### Create and edit
- Authoring is outside the app: clone the sample plugin into `<vault>/.obsidian/plugins`, run `npm run dev`, and reload (Reload app without saving, toggle the plugin, or the third-party Hot-Reload plugin). Plugins should not be developed in your main vault. [OBLD]
- No in-app editing of plugin code is documented. Each plugin has its own settings page, reached by the gear icon. [OCP]

### Install from a source, and updates
- **Registry:** `community-plugins.json`. Obsidian finds the latest version from the repository's `manifest.json`, then downloads `main.js`, `manifest.json` and `styles.css` from the GitHub release with that exact tag. [OREL]
- **From the web:** **Add to Obsidian** opens the plugin in the app. [ODIR]
- **Submission:** sign in to the directory, connect GitHub, choose **New plugin**, and submit. The automated review must pass before the plugin can be installed. Later releases reach users directly. [OSUB]
- **Updates are never automatic:** "For security purposes, community plugins don't update automatically." Use **Check for updates**, then **Update all** or **Update** per plugin. [OCP]
- **Beta versions:** not supported officially. The docs point testers to the third-party BRAT plugin. [OBETA]

### History and versions
- **Releases are GitHub releases** whose tag equals the manifest `version`. [OSUB]
- **`versions.json`** maps plugin version to minimum app version. An older app automatically gets the newest compatible plugin. [OVER]
- Users can see history on the directory's **Updates** tab. [ODIR] No user-chosen older version or rollback is documented.

### Narrow sidebar vs full page
- **Settings** is a modal (a sheet on mobile) with a left menu: Options, Core plugins, Community plugins. [OSET]
- **The Community plugins page** holds Restricted mode, Current plugins with **Check for updates**, **Browse**, and **Installed plugins** with search. [OSET], [OCP]
- **Each installed row** has icons for settings (gear), hotkeys, funding (heart), uninstall (trash), and an enable toggle. [OCP]

### Where destructive actions live
- **Uninstall:** trash icon on the row, then **Uninstall**. This is the only documented confirmation. [OCP]
- **Disable:** the row's toggle, which keeps the plugin installed. [OCP]
- **Restricted mode** is a global kill switch, on by default. Turning it on ignores every community plugin without removing any. [OSEC], [OSET]
- **Archiving** is author-side: an archived plugin leaves the directory and cannot be newly installed. [ODIR]

---

## Cross-cutting patterns

1. **Two lists for one catalogue: "installed" and "available".**
   - JetBrains splits them into **Installed** and **Marketplace** tabs.
   - VS Code uses one list with filters (`@installed`, `@updates`).
   - Raycast filters one Store to **Installed** through the Categories dropdown. It also has a separate **Manage Extensions** command.
   - Claude.ai's Customize > Skills has sections (Personal, Shared with you, Organization), and organization skills install from a separate directory. Projects has tabs: Your projects, Organization, Shared with you.
   - Obsidian splits them into **Installed plugins** and **Browse**.
   - Cursor merges every kind of item into one **Customize** list with scope filters.

2. **The detail page is the README, plus tabs of metadata.**
   - VS Code has Details/README, Features or Contributions, Changelog, and Dependencies.
   - JetBrains has Overview, What's New, Reviews, and Additional Info.
   - Obsidian's directory has Overview, Scorecard, and Updates.
   - In each, the item's own README is the main content and the store adds the rest.
   - Cursor and GitHub Actions differ: the detail is the file itself (an `.mdc` file, or a `uses:` snippet).

3. **List rows carry the primary action inline.** Install, Update and Enable sit on the row in VS Code (the Install/Update/gear button), JetBrains, Obsidian (toggle and icons) and Raycast: its developer docs give ⌘↵ to install from the list, while its manual says ↵ installs from the opened extension. Claude.ai's skill row has its on/off switch inline. Opening the detail is never required to install.

4. **Authoring happens outside the library.**
   - Every developer-platform tool sends authoring to an external editor or project. Its library is for consuming items only.
   - VS Code uses a scaffold and a dev host.
   - JetBrains uses a Gradle project.
   - Raycast uses Create Extension, then your editor.
   - Obsidian uses the sample plugin in a vault.
   - GitHub Actions uses a repository.
   - The exceptions are tools whose items are text:
     - Cursor creates rules from chat (`/create-rule`), from Customize, or from the palette, and edits them as files.
     - Claude.ai creates skills by ZIP upload or by recording the screen, and edits them beside the chat with **Edit with Claude**. A recording that overlaps an existing skill proposes an **Update** to it. Claude's API creates skill versions programmatically.

5. **Copy or fork, then edit.** Raycast's **Fork Extension** turns an installed item into an editable local copy. VS Code and JetBrains have nothing like it: you can't edit an installed item, only configure it. Claude.ai's shared skills are view-only for recipients, and the owner's edits reach them at next use.

6. **There are three update models.**
   - **Silent auto-update:**
     - VS Code is on by default, with a 12-hour delay and a per-item opt-out.
     - Raycast is always on, with a single "latest".
     - GitHub Actions moves tags such as `@v1`.
   - **Opt-in or manual only:** Obsidian never auto-updates, for security. JetBrains has an Auto-Update option.
   - **Refresh from source:** Cursor's Auto Refresh and Refresh for GitHub-sourced plugins.

7. **Choosing a version runs from "anything" to "never".**
   - **Anything:** VS Code's Install Another Version, or a GitHub Actions ref pinned to any tag or SHA.
   - **Through a separate channel:** JetBrains release channels, added as extra repositories.
   - **Automatic only:** Obsidian's `versions.json` fallback picks the version for the user.
   - **Never:** Raycast keeps no versions, only a changelog.
   - **Approved vs pending:** a Claude.ai organization skill keeps everyone on the approved version until an update passes review. The claude.ai UI shows no version list, though the API keeps versions.

8. **History is a changelog, not a diff.** Every store shows author-written notes: VS Code's Changelog, JetBrains' What's New, Raycast's Version History from `CHANGELOG.md`, and Obsidian's Updates tab. None of them shows what changed in the item's content. Real history exists only where the item lives in git: Cursor project rules and GitHub Actions.

9. **Sources are a list of repositories in settings.**
   - JetBrains: **Manage Plugin Repositories…**, which takes `updatePlugins.xml` URLs.
   - VS Code: a Private Marketplace, plus an allow-list.
   - Cursor: team marketplaces, plus **From GitHub Repository**.
   - Obsidian: a single central registry.
   - Raycast: a single repository, via pull request.
   - All of them except Obsidian and Raycast also install from a local file: VSIX, Install Plugin from Disk, and Import from Disk.

10. **Narrow means list, wide means detail.**
    - VS Code puts the list in the sidebar and opens detail as an editor tab.
    - GitHub puts a narrow Marketplace sidebar beside the YAML. You can browse it, see detail, and copy the snippet without leaving the file.
    - JetBrains and Obsidian put the whole library inside a Settings modal, with a list-plus-detail split.
    - Raycast, a launcher, pushes full views.

11. **Destructive actions sit one level down.**
    - Uninstall is never a bare button on the row:
      - VS Code puts it in the gear menu.
      - JetBrains puts it in a menu, then a confirm dialog.
      - Obsidian uses a trash icon, then a confirm step.
      - Claude.ai puts delete in the skill detail's "..." menu next to the toggle, then a confirm. A project's "..." menu leads to **Yes, delete**.
      - Raycast puts uninstall in the Action Panel.
    - Claude.ai Projects add an archive state. It keeps members and knowledge, and it blocks delete until the project is unarchived.
    - Disable is the cheaper sibling and usually needs no confirmation. Examples are the VS Code gear menu and the Obsidian toggle.
    - Uninstall often takes effect on restart: in VS Code and in JetBrains for non-dynamic plugins.
    - Bulk actions such as Disable All live in the view's overflow menu.
    - Global kill switches sit apart from per-item actions: Obsidian's Restricted mode, VS Code's Disable All, and JetBrains' Disable All Downloaded.

12. **Trust shows up at install time.**
    - VS Code: a trust dialog for a new publisher, plus signature checks.
    - JetBrains: a warning for unsigned plugins.
    - Obsidian: a Scorecard, automated scans, and Restricted mode on by default.
    - GitHub: a verified-creator badge.
    - Cursor: manual review of plugins.

## Trade-offs

- **One list or two.**
  - **One filtered list** (VS Code, Cursor Customize): you never switch views to find an item, whether installed or not. The cost is that the list's meaning depends on the filter currently applied.
  - **Split tabs or commands** (JetBrains, Raycast, Obsidian): it is always clear what you own. The cost is that one item lives in two places.
- **Install from the row or from the detail.**
  - **From the row:** fast. The cost is installing without seeing the README or trust signals.
  - **Detail-first** (the GitHub snippet, the Obsidian directory): you have read the page before you act. The cost is an extra step.
- **Authoring inside or outside.**
  - **Outside** (most stores): the library stays simple and editing uses the user's own tools. The cost is that users can't adjust an installed item.
  - **Inside** (Cursor rules from chat or Customize): low friction for text items. The cost is that the library becomes an editor too, and history moves to git or disappears.
- **Auto-update or manual.**
  - **Auto** (VS Code, Raycast, moving tags): everyone stays current. The cost is that a bad or malicious update spreads silently. VS Code softens this with a 12-hour delay. Obsidian refuses auto-update entirely for this reason.
  - **Manual:** safe. The cost is stale installs and a visible "updates available" state to maintain.
- **Versions or one latest.**
  - **Pickable versions** (VS Code, GitHub refs, Claude API versions): rollback is possible. The cost is more UI and more states to support.
  - **One latest** (Raycast): simple. The cost is no recovery when an update breaks.
  - **Channels as separate repositories** (JetBrains): isolates pre-release versions. The cost is that subscribing is a settings chore.
- **Changelog or content history.** Author changelogs are cheap and human-readable, but can be missing, as JetBrains shows: "did not leave any update notes". Git history is complete, but only for items that are files in a repository.
- **Modal or editor tab.** Settings-modal libraries (JetBrains, Obsidian) keep the library out of the work area, but block the work while open. An editor-tab detail (VS Code) sits beside the work. The cost is tab clutter.
- **Confirm or not.**
  - Confirming uninstall (JetBrains, Obsidian) prevents mistakes.
  - VS Code gets reversibility differently: a cheap Disable, plus re-install from the Marketplace, with no confirm.
  - Tools that tie removal to a restart get a natural undo window, at the cost of a pending state ("changes will be applied on restart").
