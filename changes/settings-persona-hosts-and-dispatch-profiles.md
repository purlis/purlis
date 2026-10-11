### Added

- **Add and remove a persona's hosts in Settings.** Settings › Project › Sandbox has a Persona
  hosts page. It lists each persona's `[sandbox.personas.<persona>] hosts` with Remove, and its
  Add form takes a persona of the project and a host. The host is checked as you type it, as the
  project's own hosts are. Adding a host grants nothing by itself: a chat as that persona
  reaches the list only after the person on each machine allows the list as it stands, and a
  list that changes must be allowed again. The page also says which listed hosts the machine's
  policy keeps every chat from (#1407).

- **See the profiles listed for dispatched chats.** Settings › Project › Dispatch shows each
  persona's `[dispatch.profiles]` list next to the persona's own profile, read-only. Change the
  lists with Edit as TOML (#1522).

### Changed

- **Add an override's picker is the settings set's own select.** On Settings › Project ›
  Dispatch it now looks and behaves like every other select in Settings (#626).
