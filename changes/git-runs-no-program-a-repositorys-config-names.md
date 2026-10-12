### Security

- **The git purlis runs starts no program a repository's own config names.** A repository's
  config can name a filter, a diff or merge driver, a signer, an editor, a trailer's command, or
  what a remote starts at the other end. purlis's own git turned off only the repository's hooks
  and fsmonitor. Now it turns off every one of these that a repository sets. Your own global and
  system config is not affected, and the standard Git LFS filter still runs. Where turning a
  program off cannot leave the command working, purlis stops instead: a merge that needs the
  repository's merge driver stops on a conflict, a signed save in a repository that names its own
  signer is refused, and a fetch or push in a repository that names a remote's program is not
  run (#1805).
