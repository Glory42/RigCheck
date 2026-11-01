"""Builds the parser corpus: raw PC requirement HTML from Steam's store API.

Dev-only. The app and tests never fetch; they read corpus.json.

    python3 data/fixtures/requirements/fetch.py

Re-running re-fetches everything (one request every 1.5 s) and overwrites
corpus.json. Unknown or removed app IDs are skipped.
"""

import json
import pathlib
import time
import urllib.request

USER_AGENT = "rigcheck-dev/0.0 (corpus builder; +https://github.com/Glory42/rigcheck)"

# Varied on purpose: new AAA, old classics, indies, sims, strategy, VR and
# games whose pages are known to be odd.
APP_IDS = [
    10, 70, 220, 240, 400, 440, 500, 550, 570, 620, 730, 2280, 3590, 4000, 4700,
    8930, 12100, 12120, 12210, 21000, 22380, 39210, 47890, 72850, 105600, 218620,
    227300, 230230, 230410, 236390, 236850, 244210, 250900, 252490, 255710, 264710,
    268910, 270880, 271590, 275850, 281990, 289070, 292030, 294100, 322330, 346110,
    359550, 367520, 374320, 377160, 379720, 381210, 391540, 394360, 413150, 427520,
    435150, 444090, 489830, 504230, 524220, 546560, 550650, 570940, 578080, 582010,
    582160, 588650, 594570, 601150, 646570, 739630, 782330, 805550, 812140, 814380,
    848450, 863550, 883710, 892970, 945360, 949230, 952060, 976730, 990080,
    1063730, 1066890, 1085660, 1086940, 1091500, 1097150, 1113560, 1142710, 1144200,
    1145350, 1145360, 1150690, 1151640, 1158310, 1172380, 1172470, 1174180, 1196590,
    1203220, 1222140, 1222670, 1237970, 1238810, 1238840, 1240440, 1245620, 1250410,
    1259420, 1293830, 1325200, 1328670, 1426210, 1446780, 1449850, 1462040, 1517290,
    1551360, 1593500, 1599340, 1623730, 1659040, 1665460, 1716740, 1774580, 1782210,
    1794680, 1817070, 1868140, 1888930, 1938090, 1966720, 2050650, 2208920, 2215430,
    2246340, 2357570, 2379780, 2399830, 2420110, 2440510,
]


def fetch(app_id):
    url = f"https://store.steampowered.com/api/appdetails?appids={app_id}&l=english&cc=us"
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        entry = json.load(response)[str(app_id)]
    if not entry.get("success"):
        return None
    data = entry["data"]
    requirements = data.get("pc_requirements")
    if not isinstance(requirements, dict) or not requirements.get("minimum"):
        return None
    return {
        "app_id": app_id,
        "name": data["name"],
        "minimum": requirements.get("minimum"),
        "recommended": requirements.get("recommended"),
    }


def main():
    corpus = []
    for app_id in APP_IDS:
        try:
            game = fetch(app_id)
        except Exception as error:  # keep going; report at the end
            print(f"{app_id}: {error}")
            game = None
        if game:
            corpus.append(game)
            print(f"{app_id}: {game['name']}")
        time.sleep(1.5)
    out = pathlib.Path(__file__).with_name("corpus.json")
    out.write_text(json.dumps(corpus, indent=1, ensure_ascii=False) + "\n")
    print(f"{len(corpus)} games -> {out}")


if __name__ == "__main__":
    main()
