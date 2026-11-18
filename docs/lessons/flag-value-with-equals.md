# Join an outside value to its flag with = in a pasteable command

## What happened

The web diff pane provided a pasteable terminal command, `moai diff <id> --scope <scope> --path '<file>'`. Although the page shell-quoted the path, paths starting with `-` still broke execution: the command's argument parser interpreted the quoted value as an option flag (review moai-dy7a.zsa, finding 8).

## Why

Shell quoting protects a value from shell expansion, not from the command's internal argument parser. Once the shell strips the quotes, `--path -x` is passed to the parser as two separate arguments, causing the second word to be parsed as a flag.

## Do this instead

- When constructing a command line for users to paste, join the option flag and external value with `=`: `--path='<file>'`. This prevents the argument parser from interpreting the value as a separate flag.
- Verify the command by executing it through `sh` using paths that begin with `-`, paths containing spaces, and paths containing `'`.

Learned in: moai-dy7a.zsa
