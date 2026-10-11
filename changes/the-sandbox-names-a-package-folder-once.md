### Changed

- **A sandboxed chat's project protects the same scripts with far fewer entries.** Where a
  project's hooks load packages from `node_modules` (`node -r dotenv/config`, `--import tsx`),
  the sandbox now protects each package's folder once instead of each file in it, and a
  missing folder such a name starts with once instead of each file below it. A path a protected
  folder already holds is not listed again. Every path protected before is still protected, and
  a little more is. A large monorepo's hooks name about a quarter of what they did, so a
  project is much further from the limit past which its chat is refused (#1425).
- **purlis's network record keeps a share per chat.** One chat's lines past a fifth of the
  record now let go of that chat's own oldest lines, never another chat's, so one busy chat
  cannot push the others' counts out. `purlis doctor` counts purlis's own sandbox blocks at
  most once per chat a day (#1353).

### Fixed

- **A refusal by purlis's egress proxy is answered at once.** The proxy kept telling the app
  about a refused host before it answered the client, so a slow listener held up the client's
  refusal and one of the proxy's few connection slots. It is now kept in the proxy's record
  first, answered, and told by a thread of its own (#1353).
