# AGENTS.md

Follow the development guide in [CLAUDE.md](CLAUDE.md) for architecture and
workflow.

## Comment Style

Comments are a last resort; the code is the primary source of truth.

- Never restate what the code does. If a comment is needed to understand a
  line, fix the names or structure instead.
- Don't document differences with, or behavior inherited from, the reference
  client. This is not a reimplementation diary.
- Module docs: at most a few lines. A binary-format table is only warranted if
  the parse code is genuinely hard to follow.
- Doc comments on public items: one line unless there is a real contract to
  pin down (e.g., a Rust struct that must stay byte-compatible with a WGSL
  uniform).
- Keep comments only where the code hides something non-obvious:
  coordinate-space conversions, magic constants, format quirks,
  layout/alignment invariants, or a deliberate deviation from the obvious
  approach (e.g., avoiding an optional device feature).
- In tests, comments may annotate fixture data; they should not narrate the
  assertions.

Rule of thumb: if deleting a comment loses no information, delete it.
