# Preserve drawn-state boundaries when batching input

## What happened

- A TUI loop began draining keys together with pointer events before drawing.
- A click after wheel scrolling was held for a new frame, so it hit a row the person had not seen when clicking.
- Keys after layout or focus switches read rectangles and focus corrections that drawing had not supplied yet.

## Why

- Drawing establishes state used by later input, including hit rectangles, chart geometry and focus when a pane cannot fit.
- Event type alone does not decide whether a frame boundary is safe to remove.
- Tests calling the underlying mouse handler bypassed the new queue boundary where the bug lived.

## Do this instead

- Before batching events, identify every input handler that reads state established by drawing.
- End a burst when layout or focus changes require a new frame for following keys.
- After a key changes the screen, hold following pointer events until that key has drawn; keep wheel-to-click hit testing against the frame the person saw.
- Exercise mixed event sequences through the actual burst consumer, including wheel then click, key then wheel, and layout or focus switch then navigation.

Learned in: moai-rycs.u42
