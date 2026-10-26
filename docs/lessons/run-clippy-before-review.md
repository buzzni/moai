# Run the CI clippy gate before you call the review

## What happened

moai-w25x split the worker steps into two files and moved several tests' text to
the second file. One test kept a `let work = &brief;` it no longer read. `cargo test`
passed, so the epic went to its review; the review found that CI's gate,
`cargo clippy --all-targets --workspace --locked -- -D warnings`, could not compile
the test target over that unused variable.

## Why

`cargo test` treats an unused variable as a warning and runs on. CI turns every
warning into an error, and only clippy with `--all-targets` reads the test code with
those lints.

## Do this instead

Before you create the review issue, run the gate as CI runs it, through the lock:

    flock /tmp/moai-cargo.lock cargo clippy --all-targets --workspace --locked -- -D warnings

A refactor that moves text or code between places leaves names behind, and this is
the check that finds them.

Learned in: moai-w25x.54r
