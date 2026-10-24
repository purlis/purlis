### Security

- **A skill folder that is a link is never read** (Skill studio preview, #3017). A folder in a
  project's, workspace's or persona's skills that is a link, a `SKILL.md` that is one, and a
  skills folder that is one itself are left out of every chat, and the Skill studio lists each
  as "a link, so purlis doesn't read it". A chat that loads its own skills is never shown the
  names of files a link leads to. A `SKILL.md` larger than purlis reads is left out too, and
  says so. An empty choice of chat program now means the same to every part of purlis: the
  chat is Claude Code's, as when none is named.
