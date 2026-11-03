# Keep unset values distinct from skipped comparisons

## What happened
- Review finding 3 on moai-yigx.2hv found a freshness helper that treated an undefined requested time zone as an instruction to skip comparison.
- Undefined was also a valid request value: use the browser's zone. A zone picked while that read ran could leave its old result accepted as current.
## Why
- Reusing a valid data value as a control signal makes one legitimate state impossible to compare.
## Do this instead
- Pass comparison policy separately from the requested value, using an explicit boolean or a discriminated union.
- Compare the original value even when it is absent if absence has meaning in the request.
- Cover transitions from an unset value to a concrete value while a read is in flight.

Learned in: moai-yigx.2hv
