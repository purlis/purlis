### Security

- **Chats started before a pool on another machine was added are asked to restart.** Their
  sandbox still lets them read the files that pool connects with, so the Sandbox changed notice
  lists them and says why, as it does after a cluster pool is added. It asks again, in words of
  its own, when one of those files now leads to another file (#6472).
