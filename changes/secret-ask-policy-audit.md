### Security

- An administrator's policy can turn off asks for a secret a persona doesn't declare
  (`"undeclared-secret-asks": false`): the chat is told it was declined by policy and you aren't
  asked, and Settings shows the lock under Secret asks. Each step of a Secret ask is recorded
  in the event log as metadata only (no value, name or purpose), and the chat's stated purpose
  is masked wherever it's shown or kept.
