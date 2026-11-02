# English. Every other .ftl file needs these same message IDs (tests check).
# Design D's labels are uppercase: write them uppercase here rather than
# uppercasing in code, which gets Turkish İ/ı wrong.

tab-library = LIBRARY
tab-catalog = CATALOG
tab-my-rig = MY RIG
tab-settings = SETTINGS

coming-soon = Coming soon.
detecting-hardware = Detecting your hardware…

my-rig-title = MY RIG
copy-as-text = COPY AS TEXT
label-video-memory = VIDEO MEMORY
label-vulkan = VULKAN
label-distro = DISTRO
label-kernel = KERNEL
unknown = Unknown
unknown-gpu = Unknown GPU
vram-shared = Shares system RAM
vram-dedicated = { $size } VRAM
vram-unknown = VRAM unknown
note-nouveau = This GPU uses the open-source nouveau driver, which runs most 3D games poorly. Install NVIDIA's driver to play.
note-no-vulkan = Vulkan wasn't detected. Install vulkan-tools (it provides vulkaninfo) and restart rigcheck.

appearance-title = APPEARANCE
appearance-description = Colors for the whole app. Saved for next time.
selected = SELECTED
theme-paper = PAPER
theme-ink = INK
theme-midnight = MIDNIGHT
theme-plum = PLUM
theme-forest = FOREST

language-title = LANGUAGE
language-description = The language rigcheck speaks. Game names and Steam's requirements stay in English.
language-system = SYSTEM DEFAULT

library-search = Search your games
library-scanning = Looking for your Steam games…
stamp-recommended = READY
stamp-minimum = LOW
stamp-below = TOO WEAK
stamp-blocked = BLOCKED
stamp-unknown = ???
stamp-no-data = NO DATA
stamp-checking = CHECKING

game-back = ◀ LIBRARY
game-your-pc = YOUR PC
game-minimum = MINIMUM
game-recommended = RECOMMENDED
game-protondb = PROTONDB
game-anticheat = ANTI-CHEAT
game-deck = STEAM DECK
game-native-linux = NATIVE LINUX
game-none = None
row-cpu = CPU
row-gpu = GPU
row-ram = RAM
row-vram = VRAM
row-storage = STORAGE
row-api = GRAPHICS API
why-recommended = Meets the recommended specs.
why-minimum = Runs at low settings. Your { $part } is below recommended.
why-below = Your { $part } is below the minimum.
why-blocked-anticheat = { $name } doesn't allow Linux.
why-blocked-proton = ProtonDB rates it Borked: it doesn't run through Proton.
why-unknown = Not enough data to say.
why-no-data = This game isn't in rigcheck's database yet.
part-cpu = CPU
part-gpu = GPU
part-ram = RAM
part-vram = video memory
part-storage = free disk space
part-api = Vulkan version

override-title = OVERRIDE
override-description = Correct a detection mistake, or try "what if I had…". Verdicts use your pick.
override-placeholder = Type to pick another…
override-reset = RESET

download-title = Downloading the game database…
download-progress = { $done } of { $total } MB
download-failed = Couldn't download the game database. Check your connection.
download-retry = RETRY
download-needs-newer-app = The game database needs a newer rigcheck. Update the app to get it.
stamp-not-checked = NOT CHECKED YET
why-not-checked = rigcheck hasn't read this game's store page yet.
catalog-search = Search every Steam game
game-back-catalog = ◀ CATALOG
filter-native-linux = NATIVE LINUX
filter-deck-verified = DECK VERIFIED
filter-no-anticheat-issues = NO ANTI-CHEAT ISSUES
filter-can-run = CAN RUN
filter-protondb-any = ANY
game-result = RESULT
row-meets-recommended = MEETS RECOMMENDED
row-meets-minimum = MEETS MINIMUM
row-below-minimum = BELOW MINIMUM
row-not-compared = NOT COMPARED
game-steam-text = STEAM'S REQUIREMENTS
game-store = STEAM STORE
