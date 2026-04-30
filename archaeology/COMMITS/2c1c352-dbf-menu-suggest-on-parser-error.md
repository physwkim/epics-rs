---
sha: 2c1c35268eccfc317f0e818112b1e5b67815c898
short_sha: 2c1c352
date: 2021-02-05
author: Michael Davidsaver
category: type-system
severity: low
rust_verdict: partial
audit_targets:
  - crate: base-rs
    file: src/server/database/static_lib.rs
    function: put_string_suggest
tags: [dbd-parser, menu, device-support, error-message, usability]
---
# DBF_MENU/DEVICE: missing "did you mean" suggestion on parse error

## Root Cause
When the DBD parser encountered an invalid string value for a `DBF_MENU` or
`DBF_DEVICE` field, it printed an error message but gave no hint about valid
choices. The `dbPutStringSuggest` function did not exist. Operators loading
`.db` files with typos in `field(TYPE, "")` stanzas had no guidance.

## Symptoms
On IOC startup with a malformed `.db` file containing a misspelled menu
choice, the error output is:
```
Can't set "record.FIELD" to "BadValue" ... S_db_badChoice
```
with no indication of what valid choices are. Debugging required manual
inspection of the DBD definition.

## Fix
Added `dbPutStringSuggest(DBENTRY*, const char*)` to `dbStaticLib.c`. It:
1. Checks if the field is `DBF_MENU` or `DBF_DEVICE`.
2. Iterates all valid choices.
3. Uses `epicsStrSimilarity()` to find the closest match.
4. Prints `Did you mean "BestMatch"?` if any choice has non-zero similarity.

Called from `dbRecordField` in `dbLexRoutines.c` after the error print.

## Rust Applicability
Partial. base-rs will need a DBD parser that validates field values against
menu/device type choices. A `did_you_mean`-style suggestion on parse error is
a UX feature, not a bug, but the underlying logic (iterating choices and
finding the closest match) is analogous. The `edit-distance` or similar crate
would serve the purpose. Not a correctness bug.

## Audit Recommendation
No correctness audit needed. If base-rs implements a DBD parser with field
validation, consider adding a fuzzy-match suggestion for menu/device fields
to improve operator experience.

## C Locations
- `modules/database/src/ioc/dbStatic/dbStaticLib.c:dbPutStringSuggest` — new function, similarity-based suggestion
- `modules/database/src/ioc/dbStatic/dbLexRoutines.c:dbRecordField` — calls dbPutStringSuggest on error
