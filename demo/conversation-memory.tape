## Arc 1 — "Search your past Claude conversations" (★ recommended)
##
## Render: vhs demo/conversation-memory.tape
## Output: demo/conversation-memory.gif

Output demo/conversation-memory.gif

Set FontSize 18
Set Width 1280
Set Height 720
Set Theme "Catppuccin Mocha"
Set PlaybackSpeed 1.0
Set TypingSpeed 60ms

# A subtle title comment via shell echo
Type "# Your past Claude conversations, now searchable."
Sleep 1.5s
Ctrl+U

# Show the export file exists
Type "ls samples/conversations/"
Sleep 300ms
Enter
Sleep 1.5s

# Ingest the export — progress bar visible
Type "ucp-local ingest-conversations samples/conversations/claude-conversations.json"
Sleep 500ms
Enter
Sleep 4s

# Pause for emphasis
Sleep 1s

# The killer question — pulls from a conversation that happened months ago
Type "ucp-local ask 'what did I learn about Kafka rebalancing pauses?'"
Sleep 800ms
Enter
Sleep 8s

# Let the answer settle on screen
Sleep 3s

# Optional: a second question to show it isn't a one-off
Type "ucp-local ask 'how do I sign a Rust CLI for macOS without paying Apple?'"
Sleep 800ms
Enter
Sleep 8s

Sleep 3s
