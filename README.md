# twatch

Temperature monitoring and graphing tool.

## Commands

| Command          | Description                                |
| ---------------- | ------------------------------------------ |
| `twatch`         | List recorded sessions (default)           |
| `twatch run`     | Start a recording session (250 captures by default) |
| `twatch run -t`  | Temperature-triggered recording            |
| `twatch run -c N`| Recording limited to N captures            |
| `twatch graph [ID...]` | Plot sessions in a matplotlib window (latest if omitted) |
| `twatch temp`    | Show current CPU temperature               |
| `twatch completions <shell>` | Generate shell completions (bash, zsh, fish) |

## Run options

| Option          | Description                                            |
| --------------- | ------------------------------------------------------ |
| `-d, --delay <ms>` | Milliseconds between captures (default: 250)         |
| `-i, --initial <C>` | Start temperature for `--by-temperature` (default: 40) |
| `-e, --end <C>` | Stop temperature for `--by-temperature` (default: 70)  |
| `--sensor <name>` | Target sensor for `--by-temperature`: cpu, gpu, nvme  |
| `--json`        | Output JSON records to stdout instead of TUI           |
| `--no-graph`    | Skip launching the graph after a session               |
| `--max-temp <C>`| Max Y-axis temperature (default: 110)                  |
| `--temp-steps <N>` | Grid step interval on the Y-axis (default: 5)       |

## Plot colors

- CPU = red
- GPU = green
- Other sensors = 50% opacity gray
