# Use platform types at Unix FFI boundaries

## What happened
- A tty comparison compiled on x86_64 Linux but compared `u64` from Rust metadata with `libc::stat::st_rdev`, whose Apple type is `i32`.
- The stdout pty test assumed `c_char = i8` and const pointer parameters; aarch64 Linux and Apple use different declarations.

## Why
- `cfg(unix)` selects several operating systems and architectures. It does not make their libc field types, signedness or pointer mutability identical.
- The available Linux and Windows checks could not compile the affected Apple and aarch64 Unix branches.

## Do this instead
- Prefer the standard library on both sides of a comparison, such as `MetadataExt::rdev()`, when it exposes the needed operation.
- At unavoidable FFI calls, use libc aliases such as `libc::c_char` and match the target's pointer declarations; avoid guessing primitive widths and signedness.
- Check the supported targets' declarations when a local cross-build is unavailable, and state that verification limit in the handoff.

Learned in: moai-blkz.4k2
