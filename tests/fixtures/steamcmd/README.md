# steamcmd transcripts

What steamcmd printed in experiment E4 (issue #6, runs on 2026-10-20), replayed by the fake steamcmd in `tests/steamcmd.rs`.

- The first line is the exit code (`exit=N`).
- `{user}` is replaced by the `+login` user name, and `^[` by the escape character (steamcmd prints ANSI codes on Linux).
- The SteamID is replaced by `[U:1:12345678]`. Item IDs are the deleted E4 test item's.
- `depot_build_236850.log` is synthetic: E4 recorded only the success lines of that log. The fake writes it for the failure scenarios.
- `logs-new/` and `logs-unchanged/` are the `workshop_log.txt` and `depot_build_236850.log` of E2E run 39497323643 (issue #39): an update with a changed file, then the same content again. The fake writes them for the `update` and `unchanged` scenarios; `unchanged` prints the `update` transcript, because stdout is the same.
