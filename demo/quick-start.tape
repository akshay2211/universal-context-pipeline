## Arc 3 — "From folder to grounded LLM in 60 seconds"
##
## Render: vhs demo/quick-start.tape
## Output: demo/quick-start.gif
##
## Best for docs / README "getting started" section. Less viral than Arcs 1/2,
## more practical.

Output demo/quick-start.gif

Set FontSize 18
Set Width 1280
Set Height 760
Set Theme "Catppuccin Mocha"
Set PlaybackSpeed 1.0
Set TypingSpeed 55ms

Type "# Install once. Index a folder. Ask a question."
Sleep 1.5s
Ctrl+U

# Step 1 — install (faked for the demo; real installs are obvious)
Type "# 1. Install (one-time)"
Sleep 200ms
Enter
Type "brew install ollama poppler"
Sleep 400ms
Enter
Sleep 1.2s
Type "ollama pull nomic-embed-text llama3.2"
Sleep 400ms
Enter
Sleep 1.5s
Type "cargo install ucp-local"
Sleep 400ms
Enter
Sleep 1.5s
Ctrl+L

# Step 2 — index
Type "# 2. Index a folder"
Sleep 200ms
Enter
Type "ucp-local index samples/notes samples/code"
Sleep 400ms
Enter
Sleep 5s

# Step 3 — ask
Ctrl+L
Type "# 3. Ask a question — grounded answer with citations"
Sleep 200ms
Enter
Type "ucp-local ask 'how should I handle errors in a Rust CLI?'"
Sleep 800ms
Enter
Sleep 9s

Sleep 3s
