# Build pasted commands from configured arguments

## What happened

- Review findings 1 and 2 on moai-4v3h.jkj found recovery commands that interpolated a column without shell quoting and always used `todo` to put work down.
- With `statuses = "backlog, 하는 중, done"`, pasting the current-column command split `하는 중` into two arguments, and the put-down command named a column the repository did not have.
- Ordinary default-column tests passed because those defaults contain no shell separators and really start with `todo`.

## Why

- Recovery text is executable guidance. Its ids and columns obey the command parser, while its surrounding explanation obeys the selected language.
- A second formatter or a hard-coded default silently loses the configuration and quoting rules already implemented by the CLI.

## Do this instead

- Build each pasted command from the current issue and configuration; obtain the first column from `Config::first_status`.
- Pass every variable shell argument through the shared `text::quoted` formatter before filling the translated sentence. Keep argument values separate from prose.
- Include a nondefault first column and a column containing spaces in the regression test, and check that the generated command preserves each as one argument.

Learned in: moai-4v3h.jkj
