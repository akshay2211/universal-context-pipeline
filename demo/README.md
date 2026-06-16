# UCP demos

Tape scripts for [Charmbracelet VHS](https://github.com/charmbracelet/vhs). Each one produces a clean GIF you can drop into the README, post to social, or embed in a launch writeup.

## Recording

```bash
brew install vhs    # or: go install github.com/charmbracelet/vhs@latest

# Render any tape file:
vhs demo/conversation-memory.tape
vhs demo/air-gap-rag.tape
vhs demo/quick-start.tape
```

Each tape writes a `.gif` next to itself.

## The three arcs

| File | Arc | Purpose |
|---|---|---|
| `conversation-memory.tape` | **Arc 1 — searchable Claude history** ★ | The killer demo. Lead with this on Twitter / Show HN. UCP's most defensible wedge. |
| `air-gap-rag.tape` | **Arc 2 — air-gap RAG** | The privacy-first story. Best for r/LocalLLaMA and regulated-industry pitches. |
| `quick-start.tape` | **Arc 3 — install → ask in 60s** | Embed in README. Practical, not viral. |

## Before you record

Every tape assumes:

1. `ucp-local` is on PATH (`cargo install --path .` from repo root).
2. Ollama is running with `nomic-embed-text` + `llama3.2` pulled.
3. The `samples/` folder is at `./samples/` relative to where you run `vhs`.
4. Run `ucp-local clear --hard --yes` first so the progress bar isn't all cache hits.

```bash
# Quick reset before filming
ucp-local clear --hard --yes
```

## Tips for clean recordings

- **Bump font size** on retinas: tape files set `FontSize 18`, but for ultra-high-resolution embeds bump to 22+ and `Width 1600`.
- **Slow the typing** for emphasis: `Set TypingSpeed 80ms` for dramatic effect, `40ms` for snappy.
- **Add a Sleep before each Type** so viewers can read what's about to happen.
- **`Ctrl+L`** clears the screen between phases — cleaner than scrolling.
- **`Ctrl+U`** wipes a typed line — useful for "narrator comment lines" you don't actually want to execute.

## Tweaks worth trying once you have the GIFs

- **Add captions** with `iMovie` or `Da Vinci Resolve` for the social cut. The raw GIF is fine for README; the social cut needs context.
- **Record a 2-min voiceover screencast** alongside the GIFs — that's what gets shared on launch day. Loom or OBS Studio both work; OBS gives better quality.
- **Pair Arc 1 with a tweet thread** that explains the gap: "ChatGPT and Claude both forget every conversation. UCP doesn't."
