### Security

- **Podman's run state is closed to chats in every session.** Where `XDG_RUNTIME_DIR` is unset,
  as over ssh, Podman's run state in `/tmp` is denied to every chat's writes too (#2896).
