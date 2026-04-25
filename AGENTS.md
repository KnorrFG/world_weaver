# Notes

- Top priority for generating code: simple and readable.
- Prefer plain `if`-based control flow over `break`/`continue`.
- Only use `break` or `continue` when there is a strong reason and it clearly improves the code.
- If `if` nesting starts to get hard to read, prefer helper functions over jumps, but only when the helper actually removes meaningful complexity. Do not extract trivial one-step helpers that do not pay for themselves.
- When changing a piece of code, do not stop at the narrowest possible diff if nearby code in the same local area is made inconsistent, noisier, or stylistically outdated by the change.
- Prefer finishing the local cleanup that the change obviously calls for, as long as it stays within the same responsibility and does not broaden scope much.
- If a refactor direction is already established in a file, continue it consistently instead of mixing old and new styles.
- Do not leave half-converted code behind just to minimize diff size.
- Default to making the touched code look like it was written intentionally in one pass.
- Be conservative globally, but locally thorough.
- It is good to keep a change scoped. It is bad to keep it so scoped that the touched code becomes inconsistent or visibly patchy.
