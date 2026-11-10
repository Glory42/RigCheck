# rigcheck: domain terms

The words the code and ROADMAP.md use for rigcheck's concepts. Code names follow them.

- **games.db**: the published database of games, their requirements and the **hardware table**. Downloaded by the app and built by the pipeline (`rigcheck_gamedb::GameDb`).
- **Hardware table**: every known CPU and GPU with a score, shipped inside games.db (`rigcheck_core::Catalog`).
- **Rig**: your PC as verdicts see it. The detected hardware is matched once against the hardware table, so the names shown and the scores compared come from the same entries (`rigcheck_hardware::Rig`).
- **Swap**: parts used instead of the detected ones, by hardware-table name. An **override** ("my GPU is actually X") is a saved swap. **What if** ("what if I had X?") is a swap you try out on My Rig.
- **Record**: what rigcheck knows about one game: its requirement tiers and Linux facts (`rigcheck_gamedb::Game`). A record is **checked** once a store page has been read for it. Until then the game is known only by name: NOT CHECKED YET.
- **Players' hardware**: the CPUs and GPUs ProtonDB players ran a game on without performance problems, as the weakest and typical part of each kind (`rigcheck_core::Reported`, `reported_hardware` in games.db). Verdicts use it for a part Steam's text names no model for.
- **Lookup**: asking Steam's store for a game games.db hasn't checked. The answer is kept in **fetched.db** (`rigcheck_gamedb::Fetched`) for 30 days.
- **Lookup order** (ROADMAP §7.1): games.db if it has checked the game, else a recent lookup, else a live lookup. Every page reads records through `rigcheck_gamedb::Records`, which applies this order.
- **Verdict**: how well a game should run on a rig, per component and overall (`rigcheck_core::Verdict`). On a tile it shows as a **stamp** (READY, TOO WEAK…).
- **Account**: the user's own Steam Web API key, their signed-in SteamID, and the games Steam last listed as owned. ALL MY GAMES shows it (`app/src/account.rs`).
- **Web**: every request the app makes, in one place (`app/src/web.rs`). ROADMAP §6 lists the hosts it may reach.
