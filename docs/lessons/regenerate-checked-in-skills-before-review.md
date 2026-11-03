# Regenerate checked-in skills before review

## What happened
- A skill's generated source changed while its checked-in plugin copy remained stale. A targeted command test passed, but the plugin consistency test failed on the commit sent for review.
## Why
- The source and installed plugin are separate tracked artifacts, and a narrow behavior test does not check their synchronization.
## Do this instead
- After editing generated skill source, regenerate the checked-in plugin with the repository's generator and inspect the resulting diff.
- Run the checked-in plugin consistency test and the full required test suite on the final tree before handing it to review.

Learned in: moai-n6fs
