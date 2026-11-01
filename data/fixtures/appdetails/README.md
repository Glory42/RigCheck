# appdetails fixtures

Real responses from `https://store.steampowered.com/api/appdetails?appids=<id>&l=english&cc=us`, saved once (2026-11-01) for the pipeline's ingest tests.

| File | App | Why it's here |
|---|---|---|
| `413150.json` | Stardew Valley | A game with a native Linux build |
| `1086940.json` | Baldur's Gate 3 | A Windows-only game with both requirement tiers |
| `2138330.json` | Cyberpunk 2077: Phantom Liberty | A DLC (not a game) |
| `1675200.json` | Steam Deck | Hardware (not a game); `pc_requirements` is `[]`, not an object |
| `228980.json` | Steamworks Common Redistributables | `"success": false` |
