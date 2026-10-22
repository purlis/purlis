### Security

- **A Docker pool's check holds rootless Podman to what it asked its runtime for.** The check is
  measured against the machine's IPC, host name, cgroups, folders and devices on every engine,
  and on rootless Podman it also reads the container's runtime config (#2895).
