# Check test helpers output before comparing it

## What happened

- A driver preservation test compared a git helper result to a command without its trailing newline.

## Why

- The helper returns stdout as written, including the newline added by git.

## Do this instead

- Read the helper contract and compare the intended value with its output formatting accounted for.

Learned in: moai-uwi9.b75
