# Never end a check call with a command that fails on success

## What happened

A shell call that wrote several files ended with `grep -c '@[A-Z]*@' file` to prove no placeholder was left. Every file was right and the count was `0`, but the call came back `Exit code 1`. The transcript then read as a failure and a retry, and hid whether the second call fixed anything real.

## Why

`grep`, `test`, `diff` and `git diff --quiet` all exit non-zero on exactly the outcome a check wants: nothing found, nothing different. The last command decides the exit code of the whole call.

## Do this instead

- Before sending a call, ask what its *last* command returns when the desired state holds
- Negate or swallow such a check: `! grep -q PATTERN file && echo OK`, `grep -c PATTERN file || true`
- Or keep the check in the middle and end with a plain `echo`
