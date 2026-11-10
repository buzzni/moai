# Check nested packages in bundle license inventories

## What happened

- A vendor license check treated every esbuild input under a nested dependency as belonging to its outer package.
- Nested package versions and licenses were absent from the inventory, and a forbidden nested license would not fail the test.

## Why

- The input path was split at the first `node_modules/` segment, before the actual package that supplied the file.

## Do this instead

- Identify the package at the last `node_modules/` segment of each bundled input, including scoped names.
- Check each distinct installed package and version against the license policy and the published inventory.
- Exercise the check with nested dependencies so a missing package or unexpected license names the package and fails.

Learned in: moai-z74b.t42
