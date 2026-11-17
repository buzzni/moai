# Use directory-relative calls instead of descriptor paths

## What happened

- A directory jail treated `/dev/fd/<n>` as an open directory path and passed Linux tests, but macOS fdescfs does not support identical path resolution and child lookups.

## Why

- `/dev/fd` is a platform-specific feature, not a portable guarantee that file descriptor paths operate like standard directory paths.

## Do this instead

- Use directory-relative operations such as `openat`, `fdopendir`, and `fstatat` with retained file descriptors when directory identity must persist across renames.
- Compile and test platform-specific code on all supported operating systems; cross-compilation checks complement, but do not replace, native platform tests.

Learned in: moai-vyu7.nsy
