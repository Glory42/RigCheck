# Steam fixtures

App manifests used by `crates/steam/tests/`. Tests copy them into fake home directories.

| File | App | Source |
|---|---|---|
| `appmanifest_1150690.acf` | OMORI | Real, `LastOwner` zeroed |
| `appmanifest_21000.acf` | LEGO Batman: The Videogame | Real, `LastOwner` zeroed |
| `appmanifest_1493710.acf` | Proton Experimental | Real, `LastOwner` zeroed |
| `appmanifest_4183110.acf` | Steam Linux Runtime 4.0 | Real, `LastOwner` zeroed |
| `appmanifest_228980.acf` | Steamworks Common Redistributables | Hand-written in the same shape |

`owned_games.json` is a `IPlayerService/GetOwnedGames/v1/?include_appinfo=1` response, written by hand in the shape Valve documents (partner.steamgames.com/doc/webapi/IPlayerService), with three games and made-up icon hashes. A profile whose game details are private answers `{"response":{}}`.
