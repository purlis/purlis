### Fixed

- **The Skill studio marks a skill hidden by the router only where a chat really hides it**
  (Skill studio preview, #4215). The marks now preview a chat at the workspace's root. Such a
  chat counts no clone's repo skills, so a workspace with skills in several clones no longer
  shows plane skills as hidden when that chat lists them. The Routing page's new **Preview** line
  names the chat the marks preview. Where an extension router is chosen, it says that the marks
  are the built-in router's choice, because the extension is asked only at each chat's start.
