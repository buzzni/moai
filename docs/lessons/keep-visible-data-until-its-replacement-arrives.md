# Keep visible data until its replacement arrives

## What happened

- Invalidating the detail history cache removed the selected row's journal while a background refresh was running.
- The section disappeared for the duration of the read, and fitting the shorter detail permanently pulled a reader's scroll position upward.
- A write overtaking an in-flight read could also discard usable history before any replacement existed.

## Why

- A changed source stamp means the displayed data needs refreshing; it does not mean the last successful read is absent.
- Clearing display data changes layout and scroll state before the refresh has supplied a new answer.

## Do this instead

- Keep the selected row's last successful data visible while its replacement loads.
- Stamp each result with the source marks measured before its read; show an older result under those marks and schedule another read when needed.
- Replace data and its derived layout together when the new result arrives.
- Test a refresh while the reader is scrolled into the section, including a write that overtakes an in-flight read.

Learned in: moai-ccow.9d4
