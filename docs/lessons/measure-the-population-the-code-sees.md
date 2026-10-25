# Measure on the rows the code actually receives

## What happened

Twice a question to the person was backed by a measurement that did not measure what the code faces. Once, normalising was measured on lines the tool wrote, where it changes nothing, while the issue targeted hand-resolved lines — the review found four ways those broke and the decision was reversed. Once, over a million snapshot lines were shown to start with `{"id":"`, but the function in question only ever runs on lines that failed to parse, and none of those had. (moai-0k2e)

## Why

A large sample with zero exceptions looks like proof. If the sample cannot reach the code — wrong population, or excluded by the function's call condition — the conclusion may survive by luck while the reason is dead.

## Do this instead

- Before measuring, write one line: which rows does this issue target, and which rows does this function actually receive (trace its `if`/`else` back to the call)
- Check that the sample passes that condition; if two populations matter, measure both and show both
- Read the comment of the function you are changing first — it often names the rows that matter
- Do not put counts that grow with every commit into comments; write the invariant and the test that holds it
