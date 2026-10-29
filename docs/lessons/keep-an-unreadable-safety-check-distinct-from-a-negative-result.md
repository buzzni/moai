# Keep an unreadable safety check distinct from a negative result

## What happened

- An ancestry check returned the same result for an unreadable process and for a walk with no multiplexer; a supervisor could send input to the wrong terminal.

## Why

- An interrupted lookup does not establish the absence of the condition the check is meant to detect.

## Do this instead

- Identify failure and incomplete results separately from a verified negative when a result controls where input or other actions go.
- Withhold the action when the check cannot establish the needed fact, and test a failed lookup as well as both successful outcomes.

Learned in: moai-xvvr.uvf
