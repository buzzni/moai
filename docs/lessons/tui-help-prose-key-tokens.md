# Check TUI help key tokens after changing prose

## What happened

- New Gantt help prose introduced a capital `A` and a semicolon immediately after a closing parenthesis.
- The help key test reads single characters as keys and splits at parentheses, so it treated both as undocumented bindings.

## Do this instead

- After changing TUI help prose, run `cargo test every_key_the_help_names_is_in_a_table` before the full suite.
- Keep ordinary prose from producing standalone key-shaped tokens. Use a sentence boundary after a parenthesis instead of leaving punctuation alone.
- Regenerate `docs/cli.md` from the rebuilt binary after the wording settles.

Learned in: moai-e9hi.ksl
