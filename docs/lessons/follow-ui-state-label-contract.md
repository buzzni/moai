# Follow the destination UI state-label contract

## What happened

- New web position state words copied the TUI direction names but omitted the brackets used by existing web state labels.
- The menu displayed right and top differently from left and bottom in all four languages.

## Why

- A shared meaning does not imply the same presentation contract on every surface.
- A source-language value alone did not establish how the receiving menu formats its states.

## Do this instead

- Compare new values with adjacent state labels in the destination UI before copying wording from another surface.
- Assert the complete rendered state label, including its punctuation, in a menu regression test.

Learned in: moai-8r2l.786
