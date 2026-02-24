Output demo.gif
Output demo.mp4

Set Shell zsh
Set FontSize 14
Set Width 1200
Set Height 700
Set Theme "Catppuccin Mocha"
Set Padding 20
Set TypingSpeed 50ms

# Start the API server in background, suppress output
# Run this tape from the project root: vhs demo.tape
Hide
Type "CLINICLAW_MOCK=true ./target/debug/cliniclaw > /dev/null 2>&1 &"
Enter
Sleep 3s
Show

# Launch TUI
Type "./target/debug/cliniclaw-tui"
Sleep 500ms
Enter
Sleep 3s

# Trigger ambient documentation
Type "n"
Sleep 6s

# Trigger order entry
Type "o"
Sleep 6s

# Trigger prior authorization
Type "p"
Sleep 6s

# Scroll up
Type "k"
Sleep 300ms
Type "k"
Sleep 300ms
Type "k"
Sleep 500ms

# Jump to bottom
Type "G"
Sleep 2s

# Clear and retrigger note (chain fires)
Type "c"
Sleep 1s
Type "n"
Sleep 10s

# Quit
Type "q"
Sleep 1s
