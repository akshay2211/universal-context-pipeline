## Arc 2 — "Air-gap RAG" (privacy-first wedge)
##
## Render: vhs demo/air-gap-rag.tape
## Output: demo/air-gap-rag.gif
##
## Recording tip: Before kicking this off, turn Wi-Fi off in the OS menubar so a
## brief shot of "no internet" can be paired with the GIF on social posts.

Output demo/air-gap-rag.gif

Set FontSize 18
Set Width 1280
Set Height 720
Set Theme "Catppuccin Mocha"
Set PlaybackSpeed 0.5
Set TypingSpeed 60ms

Type "# Zero cloud. Sensitive notes. Working answers."
Sleep 1.5s
Ctrl+U

# Confirm we really are offline (curl will fail fast)
Type "curl -s --max-time 1 https://api.openai.com >/dev/null && echo ONLINE || echo OFFLINE"
Sleep 300ms
Enter
Enter
Sleep 2s

# Index the notes folder
Type "ucp-local index samples/notes"
Sleep 500ms
Enter
Sleep 4s
Enter
# Sensitive-ish question about internal decisions — the kind you wouldn't paste into ChatGPT
Type "ucp-local ask 'why did we defer the multi-region rollout?'"
Sleep 800ms
Enter
Sleep 8s
Sleep 2s
Enter
# And: technical answer with citations
Type "ucp-local ask 'what does VACUUM FULL actually do, and when should I avoid it?'"
Enter
Sleep 800ms
Enter
Sleep 8s

Sleep 3s
