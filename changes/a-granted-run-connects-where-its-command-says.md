### Security

- A run you allowed under a Run grant no longer takes the chat's database and cache hosts and ports, its cloud region or its certificate authorities from the environment (`PGHOST`, `MYSQL_HOST`, `REDIS_HOST`, `AWS_REGION`, `SSL_CERT_FILE`, `CURL_CA_BUNDLE` and the like). Where it connects is what the command you read says. A certificate authority set only in the chat's environment doesn't reach that run, so name it in the command. A key that any persona's browser login or secret-fill line holds is never offered to a program, whichever persona's vault it's in.
