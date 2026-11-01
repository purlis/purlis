# Amended 2026-11-01: more of a profile is filtered before it reaches a runner

**Built by #3866**, under spec #2041. It extends item 1 of the 2026-10-29 amendment (*no sign-in
rides inside a settings file*), which stands. Everything here is **decided in implementation, to be
confirmed** (D-3866-1 to D-3866-8 in the pull request).

1. **A frontmatter is filtered as a settings file is.** A Markdown file's frontmatter loses every
   setting any harness's settings list names, at any depth, and its comments. One with nothing to
   remove stays byte for byte. One purlis cannot read keeps only the plain values of its `name` and
   `description`. Markdown with a byte that is not UTF-8 is filtered as the text a harness reads.
2. **A URL's user information never leaves**, in any harness's settings or any text file. Every URL
   loses it, whatever the scheme.
3. **A value shaped like a token is emptied**, in any harness's settings or a frontmatter, under
   whatever name.
4. **A command that shows a credential is removed whole**: the hook, status line, file suggester or
   server holding it, since half a command is another command. A command that reads a variable stays.
   A command is read only as written. It is not a promise about what a program it runs holds.
5. **What was removed is said by name, never by value.** A desktop logs it.
6. **Filtering a filtered file changes nothing**, so the runner's second filter keeps the digest
   of the 2026-10-31 amendment. Files a runner kept before this filter differ from what the desktop
   now has, and are replaced at the next start.
