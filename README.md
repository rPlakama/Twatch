Tasks:

[x] -- Better TUI
[x] -- Zoom In, Zoom out GTK  (now via matplotlib toolbar)
[x] -- Less commands, more rational
[x] -- Add 'Don't create graph option'
[x] -- Add 'Create TUI graph'
[x] -- Add 'Create GTK graph' into session commands
[x] -- More logical commands
[x] -- Empty Twatch shall run a session list

Usage:

  twatch               List recorded sessions
  twatch run           Start capture-limit recording (default 250 captures)
  twatch run -t        Start temperature-triggered recording
  twatch run -c <N>    Start capture-limit with N captures
  twatch run --no-graph  Skip graph after session
  twatch graph [ID]    Plot session (matplotlib, toolbar: zoom/pan/save)
  twatch temp          Show current CPU temperature
  twatch list          List sessions

Options:
  -d, --delay <ms>     Delay between captures (default: 250)
  -i, --initial <C>    Initial/trigger temperature (default: 40)
  -e, --end <C>        End temperature (default: 70)
  --no-graph           Don't show graph after session
  --max-temp <C>       Max Y-axis temperature (default: 110)
  --temp-steps <N>     Grid step interval (default: 5)

Plot colors:
  CPU = red,  GPU = green,  Other sensors = 50% opacity gray
