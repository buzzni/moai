# Build view fixtures from the fields the view reads

## What happened

- A Gantt motion test put only `deferred_at` on a fixture and expected the row to stand still.
- The view reads moai’s derived `shelved_by` to decide whether work is deferred; that field was missing from the fixture.

## Do this instead

- Build fixtures in the shape of the JSON consumed by the view, including derived fields.
- For deferred web rows, include the `shelved_by` returned by moai; use a real JSON fixture when the derivation itself matters.

Learned in: moai-e9hi.fmp
