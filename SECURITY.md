# Security

## Reporting a vulnerability

Please report security problems privately, not in a public issue:

- **Preferred:** GitHub's [private vulnerability reporting](https://github.com/Glory42/rigcheck/security/advisories/new) (Security tab → "Report a vulnerability").
- **Or email** [me@gorkemkaryol.dev](mailto:me@gorkemkaryol.dev).

Include what you found, how to reproduce it, and what an attacker could do with it. I aim to reply within a week. Once a fix is released, you'll be credited in the advisory unless you'd rather not be.

rigcheck is a one-person side project, so there's no bug bounty.

## Supported versions

Before v1.0, only the latest release gets fixes.

## What's in scope

rigcheck runs on your machine with your user's permissions. The parts worth a close look:

| Area | What it does |
|---|---|
| Files it reads | `/proc`, `/sys`, `/etc/os-release`, and Steam's folders (`libraryfolders.vdf`, app manifests, cached cover art) |
| Programs it runs | `vulkaninfo --summary` and `nvidia-smi` from `PATH` |
| Files it writes | `~/.config/rigcheck/settings.json` and `~/.local/share/rigcheck/games.db` |
| Network | None today. Later versions talk to exactly three read-only destinations (GitHub Releases, Steam's store API, Steam's image CDN), all through `crates/net`; see [ROADMAP §6](ROADMAP.md#6-privacy--network) |

Especially interesting: anything that makes the app talk to another host, run another program, or read or write outside the paths above, and anything in a downloaded `games.db` or Steam file that can do more than produce a wrong verdict.

Wrong verdicts themselves aren't security issues. Report them as a [Wrong verdict](https://github.com/Glory42/rigcheck/issues/new?template=wrong-verdict.yml) issue.
