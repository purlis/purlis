### Security

- **A runner's log never names a vault.** When a runner's host answers a brokered
  `secret exec`, its log names the program that ran, never the vault it used (#2838).
