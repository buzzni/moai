# Exercise every prerequisite of a public shape flag

## What happened

- `--no-gantt` promised to change deep JSON but was accepted for a one-level JSON listing, where it silently did nothing.
- The added refusal test initially tried to parse stdout; CLI JSON failures are written on stderr, so that assertion did not exercise the error contract.

## Do this instead

- For a mode-dependent public flag, test the valid combination and each missing prerequisite before calling it complete.
- Reject combinations in which the flag has no effect through the command's existing coded refusal path; include the corrected command in the message and help.
- For a JSON refusal, assert nonzero exit and empty stdout, then inspect the error code and correction on stderr.

Learned in: moai-ijjd.esf
