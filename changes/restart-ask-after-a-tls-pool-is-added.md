### Security

- **Chats started before a pool over Docker's TLS API was added are asked to restart.** Their
  sandbox still lets them read that pool's certificate and key, so the Sandbox changed notice
  lists them and says why, as it does after a cluster pool is added (#6472).
