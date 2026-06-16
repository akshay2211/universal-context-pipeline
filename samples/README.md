# UCP sample corpus

A small synthesized dataset used by the demos in `../demo/` and useful as a sanity check for your local install. Everything here is fake (no real client material, no real conversations) and license-clean.

## What's in here

```
samples/
├── notes/
│   ├── rust-error-handling.md       Working notes — Result, ?, anyhow, thiserror
│   ├── q3-planning-meeting.md       Synthesized meeting notes with decisions + action items
│   └── postgres-vacuum-tuning.md    Operational notes on autovacuum + bloat
├── code/
│   └── rate_limiter.rs              Token bucket implementation (~80 LOC)
└── conversations/
    └── claude-conversations.json    Synthesized 5-conversation Claude export (~20 turns)
```

## How to use

```bash
# Index everything
ucp-local index samples/notes samples/code

# Ingest the conversation history
ucp-local ingest-conversations samples/conversations/claude-conversations.json

# Try some demo questions
ucp-local search "Kafka rebalancing"
ucp-local ask "what did I figure out about Postgres VACUUM tuning?"
ucp-local ask "what were the decisions in Q3 planning?"
ucp-local ask "how do I sign a Rust binary for macOS?"
```

Each demo question is designed to have a clean answer in the corpus — useful for verifying your install and for filming demos that consistently produce good output.

## Topics covered in the conversations.json

- Kafka consumer rebalancing (3-4 turns)
- Postgres VACUUM tuning (2 turns)
- macOS code signing for Rust CLIs (2 turns)
- Designing Rust derive macros (3 turns)
- Setting up SSO at a small startup (2 turns)

## PDFs

No PDF is shipped in this folder — binary fixtures bloat the repo and stale fast. To exercise PDF support, drop any digitally-generated PDF into `samples/` and re-index. Suggested public-domain options:

- The Rust Book (rendered to PDF from mdBook output)
- Any arxiv paper, e.g. https://arxiv.org/pdf/1706.03762 (Attention Is All You Need)
- Project Gutenberg's epub-to-PDF exports

## Note on the "synthesized" claim

Everything in `notes/` and `conversations/` was hand-written for this corpus. None of it represents real client work, real conversations, or real internal documents. The technical content is accurate but generic enough to demo without confusion.
