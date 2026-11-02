# Avoid job-control signals in shell tests

## What happened

- A merge-driver wrapper test sent SIGINT to an inner shell. It passed in a foreground test run, but the review reproduced an exit of 0 when the runner inherited SIGINT as ignored.

## Why

- A shell without job control can start background jobs with SIGINT and SIGQUIT ignored. A child shell inherits that disposition, so sending either signal does not reliably exercise signal termination.

## Do this instead

- Use a signal outside those job-control dispositions, such as KILL, TERM or USR1, when testing a shell wrapper's relayed signal exit.
- Assert both the probe result and preservation of the installed driver setting. Do not rely on the wrapper's numeric exit alone to prove which path ran.

Learned in: moai-fcj3.8wg
