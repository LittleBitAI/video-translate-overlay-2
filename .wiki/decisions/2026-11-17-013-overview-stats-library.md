---
scope: project
severity: preference
triggers: []
domain: ''
title: "Revert the drawing to numbers, and align the storage section to those three columns."
pr: 13
merged: 2026-11-17
branch: "overview-stats-library"
---

# Revert the drawing to numbers, and align the storage section to those three columns.

What. `숫자로 보면` has been reverted to the same state as in #11. The three columns fill the board width, and the measurement source is on the line below. - `다시 볼 때는 번역하지 않습니다` has been aligned to those three column lines. The text is in the first two columns, and the mobile capture is in the third column (on the same line as 41 minutes) at its original size of 16rem and original aspect ratio. - The grid description in DESIGN.md has been updated to match this decision.

Why. In #12, while changing to a four-column grid, the column for the `숫자로 보면` section, which was not requested, narrowed from 408px to 295px, and the measurement source was moved to the fourth column. In the `다시 볼 때는 번역하지 않습니다` section, the mobile capture looked narrow as it was confined to one column.

Source. PR #13 · `overview-stats-library`
