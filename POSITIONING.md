# UCP Positioning

Who UCP is for, what it's used for, and where it stands against existing alternatives. Living document — update as the product evolves.

For build scope see [ROADMAP.md](ROADMAP.md). For developer-facing architecture see [CLAUDE.md](CLAUDE.md).

---

## TL;DR

UCP's two strongest wedges:

1. **Privacy-regulated workflows** — legal, medical, defense, NDA-bound IP. A single Rust binary, zero telemetry, zero cloud. The existing self-hosted alternatives (PrivateGPT, AnythingLLM) work but are heavy and friction-laden. Pair UCP with LM Studio (or Ollama via `ucp-local ask`) and the whole stack — index, embeddings, retrieval, chat model — runs fully offline.
2. **Conversation memory for AI power users** — every past Claude session becomes searchable across every future session, in *any* MCP client (Claude Desktop, Cursor, LM Studio, Zed, Continue.dev, custom agents). Nothing else in the AI tools landscape ships this cleanly.

Everywhere else there's a "good enough" cloud or local alternative. Don't try to win on code-only as a standalone tool (Cursor does it better) or on researcher UX (NotebookLM does it better). Win on the gaps — and on being a complement that *runs inside* Cursor / Claude / LM Studio rather than competing with them.

---

## Use cases by audience

### Software engineers

The most obvious fit — UCP becomes the bridge between your editor's LLM and your own code / docs / history.

- **Onboarding to a codebase.** Index the repo, ask Claude "where is auth token rotation handled?" Citations point straight to the file and line range.
- **Find code by intent, not name.** "The function that retries webhooks with exponential backoff." Hybrid search nails this — pure vector misses exact identifiers, pure grep misses intent.
- **Private docs that can't go to cloud.** Internal API docs, runbooks, post-mortems — index, ground, never upload.
- **"I remember asking Claude about this."** Ingest past `conversations.json` → searchable AI memory.
- **Cross-repo knowledge.** Index multiple project folders, use `folder_filter` to scope per question.

### Knowledge workers, researchers, academics

The sleeper fit — PDFs + notes + past conversations as a personal research assistant.

- **Research paper synthesis.** Index a folder of PDFs, query across them as if you'd read them all yesterday.
- **Personal notes as long-term memory.** Index Obsidian / Bear / plain Markdown vaults.
- **Decision archaeology.** "Why did we pick Postgres over Mongo back in March?" — query meeting notes + Slack exports + your own writeups.

### Writers & journalists

Where the privacy story really matters.

- **Drafting grounded in your own sources.** Interview transcripts + research folder + previous drafts, all in context.
- **Voice consistency.** Index your published work so the LLM picks up your style without being asked.
- **Source confidentiality.** Embargoed material, off-record interviews — never leaves the machine.

### Privacy-sensitive / regulated workflows

The "we literally cannot use ChatGPT" crowd.

- **Lawyers** — case files and client docs (bar rules often forbid uploading client material).
- **Therapists / clinicians** — session notes, treatment plans (HIPAA-aligned because nothing transits).
- **Defense / gov contractors** — classified or ITAR-restricted material.
- **Corporate legal / compliance** — NDA-bound material, M&A docs, IP-sensitive R&D.

UCP's posture (zero telemetry, zero cloud, single binary) is a *prerequisite*, not a feature.

### Solo founders & consultants

- **"Have I solved this before?"** Index every past project folder.
- **Per-client isolation.** One folder per client, `folder_filter` enforces scoping per query.
- **Personal CRM-lite.** Conversation logs + meeting notes per client.

### Educators & students

- **Textbook PDFs + lecture notes as one searchable corpus.**
- **Hallucination-resistant** because every answer cites the source.

### Open source maintainers

- Index docs + issue exports + relevant code → answer questions about your project with citations.
- Could be served to contributors as a tool.

### Power users — conversation memory

The single sleeper feature. Periodically export Claude conversations, run `ucp-local ingest-conversations`. Every Claude session has access to every prior Claude session.

No cloud tool ships this. Structural advantage of being local-first.

---

## Concrete query patterns UCP unlocks

```
"Find every place I wrote about Kalman filters in my notes."
"What did Claude tell me about Postgres VACUUM tuning three months ago?"
"Summarize the conclusions from these 12 papers on transformer attention."
"Show me where this codebase handles rate limiting."
"What's our policy on travel reimbursement?"            # index ~/work/policies/
"Find inconsistencies between my Q3 plan and Q3 status updates."
"Did I already write a function that parses HL7 messages?"
"What were the main objections in last week's customer calls?"   # index transcripts
```

---

## Workflow patterns

- **Background-indexing daemon:** `ucp-local watch ~/notes` runs continuously, index stays fresh.
- **Per-project context:** one folder = one project = one filtered slice via `folder_filter`.
- **Growing memory:** monthly Claude export → `ucp-local ingest-conversations`. Personal memory compounds.
- **Air-gap mode:** UCP + a local chat runtime (LM Studio or Ollama), no internet. Useful for flights, secure facilities, classified work, paranoid setups. Indexing, embeddings, retrieval, and the chat model all run on-device.
- **Multi-client memory:** index once, query from Claude Desktop *and* Cursor *and* LM Studio. The index is a shared substrate; the client is whichever LLM you prefer for the task.

---

## Elevator pitch by audience

| Audience | One-line pitch |
|---|---|
| Engineers | "Grep + semantic search across your code, docs, and past Claude chats — fed straight into Claude Desktop, Cursor, or whichever MCP client you live in." |
| Researchers | "Drop a folder of PDFs in. Ask Claude (or a local model in LM Studio) about them with citations. Nothing leaves your machine." |
| Writers | "A research assistant grounded in your own notes and sources, with privacy guaranteed." |
| Privacy-sensitive | "Local-first LLM grounding. Pair with LM Studio for a fully offline stack — index, embeddings, retrieval, and chat model all on-device. Zero telemetry. Single binary." |
| Power users | "Your past Claude conversations become searchable memory across every future session, in every MCP client you use." |
| Offline / air-gap users | "Index, retrieve, and chat without an internet connection. UCP + LM Studio = end-to-end RAG on a laptop in airplane mode." |

---

## Competitive map

Honest evaluation. Some rows admit UCP isn't materially better — that's deliberate, more useful than cheerleading.

| Audience | Strongest existing options | What UCP genuinely adds | Is UCP actually better? |
|---|---|---|---|
| **Software engineers** | Cursor `@codebase`, Continue.dev, Claude Code, Sourcegraph Cody ($), Aider | Cross-source unification: code + docs + past Claude chats in one query, surfaced *inside* Cursor / Claude / LM Studio via MCP. None of them touch conversation memory or cross-repo notes natively. | **Partially.** For code-only inside one repo Cursor / Claude Code win. UCP isn't a replacement — it's an MCP add-on that extends Cursor's reach into private notes, sibling repos, and past Claude history. |
| **Researchers / academics** | **NotebookLM** (free, polished), Khoj, AnythingLLM, ChatGPT w/ file upload, Obsidian + Smart Connections | True local-only. NotebookLM uploads to Google. Khoj is the closest match. | **Only if privacy matters.** NotebookLM is excellent for cloud-OK users. UCP wins on regulated / embargoed material. |
| **Writers & journalists** | Manual paste into Claude/ChatGPT, Sudowrite ($), Lex.page, NotebookLM, Scrivener (no AI) | Confidential source material never transits. Citations preserve provenance. | **Yes, for sensitive sources.** For non-sensitive writing, ChatGPT + upload is faster. |
| **Privacy-sensitive / regulated** | Self-hosted LLM (LM Studio or Ollama, no retrieval out of the box), AnythingLLM (local mode), PrivateGPT, LibreChat + local model, on-prem Cohere / Bedrock-VPC ($$$) | Single binary, zero config, zero telemetry, MCP-native. Plugs straight into LM Studio's MCP support to give it real RAG without bolting on a vector DB. Existing locals are clunky to set up. | **Yes, materially.** UCP's strongest audience — and the LM-Studio-as-host pattern means the chat model stays local too. |
| **Solo founders / consultants** | Notion AI, Mem.ai, Reflect, Apple Notes + manual copy-paste | Per-folder scoping enforces client isolation. No risk of leaking client A into client B. | **Yes if client confidentiality is real.** Notion AI wins on UX otherwise. |
| **Educators & students** | **NotebookLM** (free, purpose-built), Khanmigo, ChatGPT w/ uploaded PDFs | Local-only means no per-student account, no upload limits. | **No, honestly.** NotebookLM is engineered for this. UCP only wins if school IT bans cloud. |
| **OSS maintainers** | DeepWiki (auto-AI-docs from repos), Cody, Sourcegraph, Mintlify | Self-hostable Q&A grounded in your repo. No vendor lock-in. | **Equal.** DeepWiki has more momentum; UCP wins on full self-host. |
| **Power users — conversation memory** | Claude native memory (beta, opaque), ChatGPT memory (cloud, opaque), Pieces.app ($), Letta / Memgpt (academic), mem0 (library) | A real, searchable archive of every past Claude conversation, citable in any session. No cloud opacity, no quotas. | **Yes, structurally.** Nothing else fills this cleanly. Most defensible UCP use case. |

---

## Strategic takeaways

1. **Two genuine wedges:** privacy-regulated workflows + conversation memory for power users. Everywhere else has a "good enough" alternative.
2. **Don't try to outcompete NotebookLM head-on.** For cloud-OK researchers / students, recommend NotebookLM, not UCP.
3. **Don't try to outcompete Cursor on code-only.** UCP for engineers wins on *combination* (code + docs + Claude chats), not on code alone — and it wins from *inside* Cursor as an MCP server, not as a replacement.
4. **Lead the launch with conversation memory.** Most underbuilt thing in the AI tools landscape. "Your Claude chats become a searchable second brain across every future session" is a story nobody else can tell.
5. **Privacy is the most defensible long-term wedge.** Regulated industries can't move to cloud; existing self-hosted options are heavy compared to a single Rust binary. The LM Studio pairing makes "fully offline, end-to-end RAG" a one-paragraph install — that's a unique pitch.
6. **Be a complement, not a competitor.** UCP runs *inside* Claude Desktop, Cursor, and LM Studio via MCP. Framing it as an add-on that extends tools users already love is structurally easier to sell than framing it as a replacement.

---

## What UCP is explicitly NOT for

So you don't oversell.

- ❌ Real-time web search (Perplexity / Brave Search MCP / Exa own this).
- ❌ Real-time data (stock prices, news, weather).
- ❌ Multi-user / shared knowledge base (Notion + AI, Glean for enterprise).
- ❌ Image content search (deferred to v0.2+).
- ❌ Workflow automation (Zapier, n8n).
- ❌ Long document generation (the LLM does that — UCP just grounds it).

## Where UCP genuinely cannot compete

Be honest with yourself about these:

- **Real-time web data** — Perplexity, Brave Search MCP, Exa own this.
- **Multi-user team knowledge** — Notion + AI, Glean for enterprise.
- **Hand-holding for non-technical users** — NotebookLM's UX is way ahead.
- **Marketing reach** — Google ships NotebookLM in every Gmail account; UCP starts at zero.

If those audiences are the target, UCP isn't the project. If the privacy-sensitive + conversation-memory wedge sounds right, you're aiming at the actual gap.
