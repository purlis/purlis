# Amendment, 2026-11-10: opencode 1.19.5 still has no per-chat plugin switch

Ticket #3009 (SK-6c) asks for opencode to be measured again on each new release, and for an
adapter as soon as one release lets a chat turn a single plugin off. This is that measurement
for the release purlis pins today. It changes no ruling: the 2026-10-27 amendment already
removed every per-project plugin choice, so a switch would only matter for a Try it's scratch
chat and a grade, which turn every installed plugin off (decision 50, #3468). It does correct
the harness card's plugin line, which still spoke of project choices (below).

## The versions

- **opencode 1.19.5**: the version purlis pins (`ci/skill-grades/pins.toml`) and npm's
  `opencode-ai` `latest` tag on 2026-11-10. The 2026-10-21 table was measured on 1.18.33.
- **opencode 2.2.7**: the newest release on GitHub (`anomalyco/opencode`, published
  2026-11-10). It is not on npm (`opencode-ai`'s `latest` is still 1.19.5), and it is a new
  program: a background service, plugins as directories under a `plugins` key, and a new
  plugin API. purlis's adapter, its shim (ADR 0058) and its pin are all for 1.x.

## opencode 1.19.5, measured

Installed with `npm install --prefix <scratch> opencode-ai@1.19.5`. No credential and no
network sign-in: each run had `env -i` with its own `HOME`, `TMPDIR` and the four `XDG_*`
folders in a scratch folder `$W`, `OPENCODE_DISABLE_AUTOUPDATE=1`, and
`OPENCODE_CONFIG=$W/base.json` naming two file plugins:

```json
{"plugin": ["file://$W/plug/a.js", "file://$W/plug/b.js"]}
```

Each row adds a session layer in `OPENCODE_CONFIG_CONTENT`, the layer purlis already writes,
and reads `plugin` and `plugin_origins` from `opencode debug config`, run from `$W/proj` as:

```sh
env -i PATH=/usr/bin:/bin TMPDIR=$W/tmp HOME=$W/home XDG_CONFIG_HOME=$W/cfg \
  XDG_DATA_HOME=$W/data XDG_CACHE_HOME=$W/cache XDG_STATE_HOME=$W/state \
  OPENCODE_CONFIG=$W/base.json OPENCODE_DISABLE_AUTOUPDATE=1 \
  OPENCODE_CONFIG_CONTENT='{"plugin":[]}' opencode debug config
```

The `{"plugin":[]}` row's output, cut to the two keys:

```json
"plugin": [],
"plugin_origins": [
  {"spec": "file://$W/plug/a.js", "source": "$W/base.json", "scope": "global"},
  {"spec": "file://$W/plug/b.js", "source": "$W/base.json", "scope": "global"}
]
```

All the rows:

| Session layer (`OPENCODE_CONFIG_CONTENT`) | `plugin` | `plugin_origins` (what `plugin/index.ts` loads) |
|---|---|---|
| none | a, b | a, b (both from `base.json`) |
| `{"plugin":["file://$W/plug/b.js"]}` | a, b | a (`base.json`), b (`OPENCODE_CONFIG_CONTENT`): concatenated |
| `{"plugin":[]}` | `[]` | a, b: still both |
| `{"plugin_origins":[]}` | a, b | a, b: the key is derived state and not read from a config |
| `{"disabled_plugins":["file://$W/plug/a.js"]}` | a, b | a, b: no such key |
| `{"plugin":{"file://$W/plug/a.js":false}}` | refused: `Expected array \| undefined` | |
| none, with `OPENCODE_DISABLE_DEFAULT_PLUGINS=1` | a, b | a, b: it drops only the built-in ones |

The same as on 1.18.33. Its source at the `v1.19.5` tag says why: `config/config.ts` merges
every config's `plugin` list into `plugin_origins` and dedupes it (`deduplicatePluginOrigins`),
and `plugin/index.ts` loads `flags.pure ? [] : cfg.plugin_origins`. The only switches are still
`--pure` and `OPENCODE_PURE` (no plugin at all, purlis's shim included) and
`OPENCODE_DISABLE_DEFAULT_PLUGINS` (the built-in ones). No release note from 1.18.34 to 1.19.5
mentions a plugin switch: the only plugin lines are a TUI fix for a plugin's name (1.18.34) and
a docs entry (1.18.35).

## opencode 2.2.7, read from its source

The 2.x documentation (`services/site/src/docs/content/plugins.mdx` at the `v2.2.7` tag,
"Control") gives what 1.x lacks: `plugins` entries are applied in order across config files,
and an id prefixed with `-` turns that plugin off (`"plugins": ["-acme.reviewer"]`; `*` and
`<prefix>.*` match several). `core/src/config/plugin/source.ts` parses the `-`, and
`core/src/plugin/supervisor.ts` drops the matched plugin, except two built-ins guarded so a
repository cannot turn policy off. `OPENCODE_CONFIG_CONTENT` is still read
(`cli/src/server-process.ts`), as a config layer above the user's and the project's.

**But it reaches only a server the chat starts itself.** By default a 2.x chat talks to one
background service per machine. That service reads its own environment when it starts, and a
second launch finds the service already running and returns (`findIncumbent`). A chat's
`OPENCODE_CONFIG_CONTENT` would reach its plugins only under `--standalone`, a private server
for that run.

**Not measured live.** Every 2.x command, `debug config` included, needs a local server. This
task's sandbox refuses a local port, so `plugin list` and `debug config --standalone` both
stopped at "Failed to start server". The binary itself ran (`opencode --version`:
`opencode v2.2.7`). What is above is read from the source and the documentation only.

## What this means

- **No adapter now.** The release purlis runs has no per-chat switch, and nothing changes for
  it: a Try it's scratch chat on opencode keeps the person's plugins on, and a grade is refused
  on opencode (`Opencode::can_grade`). opencode stays declared `per_chat_plugins = "no"`.
- **#3009 stays open.** When purlis supports opencode 2.x, measure `--standalone` with
  `OPENCODE_CONFIG_CONTENT` `{"plugins":["-<id>"]}` on a machine that allows a local port. If it
  works, `harness_plugin::OPENCODE` can list what is installed and turn each one off, as Codex
  does since 0.163.0.
- **The harness card says what is true now.** Its plugin line said a harness without the switch
  is not handed "the plugins chosen for this project". Since #3467 there are no such plugins. The
  line now reads "Turns your plugins off for one chat", and a harness without it "cannot turn a
  plugin off for one chat, so every chat on it has your plugins on". Codex is declared
  `per_chat_plugins = "yes"`, since it turns a plugin off for one chat (the 2026-10-21
  amendment). A test holds each built-in's answer to its adapter's
  `turns_a_plugin_off_for_one_chat`.
