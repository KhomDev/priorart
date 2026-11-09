# Render with vhs on PATH (and ttyd, ffmpeg, a browser):
#   PATH="/workspace/target/release:$PATH" vhs docs/demo.tape
Output docs/demo.gif
Set Shell bash
Set FontSize 14
Set Width 1200
Set Height 1100
Set Padding 16
Set TypingSpeed 20ms
Type "vg check tests/fixtures/cmp-04-report.md"
Enter
Sleep 5s
