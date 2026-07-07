---

Usage:

  twatch               List recorded sessions
  twatch run           Start capture-limit recording (default 250 captures)
  twatch run -t        Start temperature-triggered recording
  twatch run -c <N>    Start capture-limit with N captures
  twatch run --no-graph  Skip graph after session
  twatch run --graph-type tui  Show TUI graph after session
  twatch graph [ID]    Plot session in GTK window (scroll=zoom, drag=pan)
  twatch graph-tui [ID]  Plot session in terminal (arrows=pan, +/-=zoom)
  twatch temp          Show current CPU temperature
  twatch list          List sessions

Options:
  -d, --delay <ms>     Delay between captures (default: 250)
  -i, --initial <C>    Initial/trigger temperature (default: 40)
  -e, --end <C>        End temperature (default: 70)
  --no-graph           Don't show graph after session
  --max-temp <C>       Max Y-axis temperature (default: 110)
  --temp-steps <N>     Grid step interval (default: 5)
