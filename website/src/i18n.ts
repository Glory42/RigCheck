// The site's text in each language. Stamp and table words match the app's
// own translations (app/i18n/*.ftl), so the site and the app say the same.

export const languages = ["en", "tr", "de"] as const;
export type Lang = (typeof languages)[number];

/** The language's name in that language, as the app's picker writes it. */
export const nativeNames: Record<Lang, string> = {
  en: "ENGLISH",
  tr: "TÜRKÇE",
  de: "DEUTSCH",
};

export function homePath(lang: Lang): string {
  return lang === "en" ? "/" : `/${lang}/`;
}

const en = {
  title: "rigcheck: can my PC run it? Steam games on Linux",
  description:
    "A Linux desktop app that checks every Steam game against your hardware and against Linux: Proton, anti-cheat, drivers and Vulkan. No accounts, no telemetry.",
  skip: "Skip to content",
  navFeatures: "FEATURES",
  navPrivacy: "PRIVACY",
  navInstall: "INSTALL",
  language: "Language",

  heroKicker: "FOR LINUX · FREE · OPEN SOURCE",
  heroTitle: "CAN MY PC RUN IT?",
  heroLead:
    "rigcheck checks your Steam games against your hardware, and against Linux itself: Proton, anti-cheat, drivers and Vulkan. Before you buy, not after.",
  heroDownload: "DOWNLOAD",
  heroSource: "SOURCE ON GITHUB",
  heroFormats: "AppImage · .deb · .rpm · x86_64",
  shotLibrary:
    "The Library: installed Steam games as cover tiles with READY, TOO WEAK and BLOCKED stamps",

  stampsTitle: "ONE STAMP PER GAME",
  stampsLead: "Every game gets a verdict, worked out on your own PC.",
  stampReady: "READY",
  stampLow: "LOW",
  stampWeak: "TOO WEAK",
  stampBlocked: "BLOCKED",
  stampUnknown: "???",
  stampReadyText: "Meets the recommended specs.",
  stampLowText: "Meets the minimum; at least one part falls short of recommended.",
  stampWeakText: "A part is below the minimum, and rigcheck tells you which.",
  stampBlockedText: "Won't run on Linux whatever the hardware, like anti-cheat that denies Linux.",
  stampUnknownText: "Not enough data to say. rigcheck shows the raw text instead of guessing.",

  whyTitle: "IT TELLS YOU WHY",
  whyLead:
    "Every game page puts your PC next to the game's requirements and points at the part holding you back.",
  whyGame: "Baldur's Gate 3",
  whyVerdict: "Your GPU is below the minimum.",
  colPart: "PART",
  colYourPc: "YOUR PC",
  colMinimum: "MINIMUM",
  colResult: "RESULT",
  resultMinimum: "MEETS MINIMUM",
  resultBelow: "BELOW MINIMUM",
  resultRecommended: "MEETS RECOMMENDED",
  factProtondb: "PROTONDB",
  factAnticheat: "ANTI-CHEAT",
  factDeck: "STEAM DECK",
  factProtondbValue: "PLATINUM",
  factAnticheatValue: "None",
  factDeckValue: "VERIFIED",

  featuresTitle: "WHAT IT DOES",
  featLibraryTitle: "READS YOUR STEAM LIBRARY",
  featLibraryText:
    "Native, ~/.steam and Flatpak installs, and every library folder, even on other drives. Add your own Steam Web API key to see every game you own.",
  featHardwareTitle: "KNOWS YOUR HARDWARE",
  featHardwareText:
    "CPU, RAM, every GPU by its PCI ID, video memory, Vulkan and the driver, read straight from your system. Hybrid laptops use the discrete GPU.",
  featCatalogTitle: "SEARCHES ALL OF STEAM",
  featCatalogText:
    "Every game on Steam, most popular first. Filter by what your PC can run, ProtonDB tier, native Linux, Deck Verified or anti-cheat.",
  featWhatIfTitle: "ASKS “WHAT IF?”",
  featWhatIfText: "Pick a different GPU or CPU and see which games change verdict before you upgrade.",
  featLinuxTitle: "THINKS LINUX",
  featLinuxText:
    "ProtonDB ratings, anti-cheat that blocks Linux from AreWeAntiCheatYet, Steam Deck status and the Vulkan version Proton needs.",
  featOfflineTitle: "WORKS OFFLINE",
  featOfflineText:
    "The game database is downloaded once and updated in the background. The app tells you when a new version is out.",
  shotCatalog: "The Catalog: the most popular Steam games as cover tiles with verdict stamps, a search box and filter chips",

  privacyTitle: "NOTHING ABOUT YOU LEAVES YOUR PC",
  privacyLead: "No accounts, no telemetry, no servers of our own. These are the only places rigcheck talks to:",
  privacyGithub: "the game database and the new-version notice",
  privacySteamStore: "games the database hasn't checked yet",
  privacySteamCdn: "cover art",
  privacySteamApi: "your owned games, only if you add your own Steam key",
  privacySite: "This site doesn't track you either: no analytics, no cookies, no third-party fonts.",

  installTitle: "INSTALL",
  installLead: "Download the package for your distro from the latest release.",
  installAppImage: "Any distro",
  installDeb: "Debian, Ubuntu, Mint",
  installRpm: "Fedora, openSUSE",
  installTarball: "Install by hand",
  installGet: "DOWNLOAD",
  installVulkan: "Install vulkan-tools as well, so rigcheck can read your GPU's Vulkan version and video memory.",
  installSoon: "SOON",
  installAur: "rigcheck-bin for releases, rigcheck-git for the latest code.",
  installFlathub: "One click from GNOME Software and KDE Discover.",
  installAllReleases: "ALL RELEASES",

  footerLicense: "MIT license",
  footerBy: "Made by",
  footerData: "Data from Steam, ProtonDB and AreWeAntiCheatYet. Not affiliated with Valve.",
};

export type Strings = typeof en;

const tr: Strings = {
  title: "rigcheck: Bilgisayarım kaldırır mı? Linux'ta Steam oyunları",
  description:
    "Her Steam oyununu donanımına ve Linux'a göre denetleyen bir Linux masaüstü uygulaması: Proton, anti-cheat, sürücüler ve Vulkan. Hesap yok, telemetri yok.",
  skip: "İçeriğe geç",
  navFeatures: "ÖZELLİKLER",
  navPrivacy: "GİZLİLİK",
  navInstall: "KURULUM",
  language: "Dil",

  heroKicker: "LINUX İÇİN · ÜCRETSİZ · AÇIK KAYNAK",
  heroTitle: "BİLGİSAYARIM KALDIRIR MI?",
  heroLead:
    "rigcheck Steam oyunlarını donanımına ve Linux'un kendisine göre denetler: Proton, anti-cheat, sürücüler ve Vulkan. Satın almadan önce, sonra değil.",
  heroDownload: "İNDİR",
  heroSource: "GITHUB'DA KAYNAK KODU",
  heroFormats: "AppImage · .deb · .rpm · x86_64",
  shotLibrary: "Kütüphane: yüklü Steam oyunları HAZIR, YETERSİZ ve ENGELLİ damgalı kapaklar olarak",

  stampsTitle: "HER OYUNA BİR DAMGA",
  stampsLead: "Her oyun, senin bilgisayarında hesaplanan bir sonuç alır.",
  stampReady: "HAZIR",
  stampLow: "DÜŞÜK",
  stampWeak: "YETERSİZ",
  stampBlocked: "ENGELLİ",
  stampUnknown: "???",
  stampReadyText: "Önerilen gereksinimleri karşılıyor.",
  stampLowText: "Minimumu karşılıyor; en az bir parça önerilenin altında.",
  stampWeakText: "Bir parça minimumun altında ve rigcheck hangisi olduğunu söyler.",
  stampBlockedText: "Donanım ne olursa olsun Linux'ta çalışmaz; örneğin Linux'a izin vermeyen anti-cheat.",
  stampUnknownText: "Karar vermeye yetecek veri yok. rigcheck tahmin etmek yerine ham metni gösterir.",

  whyTitle: "NEDENİNİ SÖYLER",
  whyLead: "Her oyun sayfası bilgisayarını oyunun gereksinimlerinin yanına koyar ve seni geride tutan parçayı gösterir.",
  whyGame: "Baldur's Gate 3",
  whyVerdict: "GPU minimum gereksinimin altında.",
  colPart: "PARÇA",
  colYourPc: "SENİN PC'N",
  colMinimum: "MİNİMUM",
  colResult: "SONUÇ",
  resultMinimum: "MİNİMUMU KARŞILIYOR",
  resultBelow: "MİNİMUMUN ALTINDA",
  resultRecommended: "ÖNERİLENİ KARŞILIYOR",
  factProtondb: "PROTONDB",
  factAnticheat: "ANTI-CHEAT",
  factDeck: "STEAM DECK",
  factProtondbValue: "PLATINUM",
  factAnticheatValue: "Yok",
  factDeckValue: "DOĞRULANMIŞ",

  featuresTitle: "NELER YAPAR",
  featLibraryTitle: "STEAM KÜTÜPHANENİ OKUR",
  featLibraryText:
    "Yerel, ~/.steam ve Flatpak kurulumlarını ve başka disklerdekiler dahil her kütüphane klasörünü bulur. Kendi Steam Web API anahtarını eklersen sahip olduğun tüm oyunları görürsün.",
  featHardwareTitle: "DONANIMINI TANIR",
  featHardwareText:
    "İşlemci, RAM, PCI kimliğiyle her GPU, ekran belleği, Vulkan ve sürücü doğrudan sisteminden okunur. Hibrit dizüstülerde harici GPU kullanılır.",
  featCatalogTitle: "TÜM STEAM'DE ARAR",
  featCatalogText:
    "Steam'deki her oyun, en popülerden başlayarak. Bilgisayarının kaldırdığı oyunlara, ProtonDB seviyesine, yerel Linux'a, Deck doğrulamasına veya anti-cheat'e göre süz.",
  featWhatIfTitle: "“YA OLSAYDI?” DİYE SORAR",
  featWhatIfText: "Başka bir GPU ya da işlemci seç, yükseltmeden önce hangi oyunların sonucunun değiştiğini gör.",
  featLinuxTitle: "LINUX GİBİ DÜŞÜNÜR",
  featLinuxText:
    "ProtonDB puanları, AreWeAntiCheatYet'ten Linux'u engelleyen anti-cheat'ler, Steam Deck durumu ve Proton'un istediği Vulkan sürümü.",
  featOfflineTitle: "ÇEVRİMDIŞI ÇALIŞIR",
  featOfflineText:
    "Oyun veritabanı bir kez indirilir ve arka planda güncellenir. Yeni sürüm çıktığında uygulama haber verir.",
  shotCatalog: "Katalog: en popüler Steam oyunları sonuç damgalı kapaklar, arama kutusu ve filtrelerle",

  privacyTitle: "HAKKINDA HİÇBİR ŞEY BİLGİSAYARINDAN ÇIKMAZ",
  privacyLead: "Hesap yok, telemetri yok, kendi sunucumuz yok. rigcheck'in konuştuğu yerler yalnızca bunlar:",
  privacyGithub: "oyun veritabanı ve yeni sürüm bildirimi",
  privacySteamStore: "veritabanının henüz bakmadığı oyunlar",
  privacySteamCdn: "kapak görselleri",
  privacySteamApi: "sahip olduğun oyunlar, yalnızca kendi Steam anahtarını eklersen",
  privacySite: "Bu site de seni izlemez: analitik yok, çerez yok, üçüncü taraf yazı tipi yok.",

  installTitle: "KURULUM",
  installLead: "Dağıtımına uygun paketi son sürümden indir.",
  installAppImage: "Her dağıtım",
  installDeb: "Debian, Ubuntu, Mint",
  installRpm: "Fedora, openSUSE",
  installTarball: "Elle kurulum",
  installGet: "İNDİR",
  installVulkan: "vulkan-tools paketini de kur; böylece rigcheck GPU'nun Vulkan sürümünü ve ekran belleğini okuyabilir.",
  installSoon: "YAKINDA",
  installAur: "Sürümler için rigcheck-bin, en güncel kod için rigcheck-git.",
  installFlathub: "GNOME Yazılım ve KDE Discover'dan tek tıkla.",
  installAllReleases: "TÜM SÜRÜMLER",

  footerLicense: "MIT lisansı",
  footerBy: "Yapan:",
  footerData: "Veriler Steam, ProtonDB ve AreWeAntiCheatYet'ten. Valve ile bağlantısı yoktur.",
};

const de: Strings = {
  title: "rigcheck: Läuft das auf meinem PC? Steam-Spiele unter Linux",
  description:
    "Eine Linux-Desktop-App, die jedes Steam-Spiel mit deiner Hardware und mit Linux abgleicht: Proton, Anti-Cheat, Treiber und Vulkan. Keine Konten, keine Telemetrie.",
  skip: "Zum Inhalt springen",
  navFeatures: "FUNKTIONEN",
  navPrivacy: "DATENSCHUTZ",
  navInstall: "INSTALLATION",
  language: "Sprache",

  heroKicker: "FÜR LINUX · KOSTENLOS · OPEN SOURCE",
  heroTitle: "LÄUFT DAS AUF MEINEM PC?",
  heroLead:
    "rigcheck gleicht deine Steam-Spiele mit deiner Hardware ab, und mit Linux selbst: Proton, Anti-Cheat, Treiber und Vulkan. Vor dem Kauf, nicht danach.",
  heroDownload: "HERUNTERLADEN",
  heroSource: "QUELLCODE AUF GITHUB",
  heroFormats: "AppImage · .deb · .rpm · x86_64",
  shotLibrary: "Die Bibliothek: installierte Steam-Spiele als Cover mit den Stempeln BEREIT, ZU SCHWACH und BLOCKIERT",

  stampsTitle: "EIN STEMPEL PRO SPIEL",
  stampsLead: "Jedes Spiel bekommt ein Urteil, berechnet auf deinem eigenen PC.",
  stampReady: "BEREIT",
  stampLow: "NIEDRIG",
  stampWeak: "ZU SCHWACH",
  stampBlocked: "BLOCKIERT",
  stampUnknown: "???",
  stampReadyText: "Erfüllt die empfohlenen Anforderungen.",
  stampLowText: "Erfüllt das Minimum; mindestens ein Teil liegt unter der Empfehlung.",
  stampWeakText: "Ein Teil liegt unter dem Minimum, und rigcheck sagt dir, welches.",
  stampBlockedText: "Läuft unter Linux mit keiner Hardware, etwa wegen Anti-Cheat, das Linux sperrt.",
  stampUnknownText: "Zu wenige Daten für ein Urteil. rigcheck zeigt den Originaltext, statt zu raten.",

  whyTitle: "ES SAGT DIR, WARUM",
  whyLead: "Jede Spielseite stellt deinen PC neben die Anforderungen des Spiels und zeigt, welches Teil dich bremst.",
  whyGame: "Baldur's Gate 3",
  whyVerdict: "Deine GPU liegt unter dem Minimum.",
  colPart: "TEIL",
  colYourPc: "DEIN PC",
  colMinimum: "MINIMUM",
  colResult: "ERGEBNIS",
  resultMinimum: "ERFÜLLT MINIMUM",
  resultBelow: "UNTER MINIMUM",
  resultRecommended: "ERFÜLLT EMPFOHLEN",
  factProtondb: "PROTONDB",
  factAnticheat: "ANTI-CHEAT",
  factDeck: "STEAM DECK",
  factProtondbValue: "PLATINUM",
  factAnticheatValue: "Keins",
  factDeckValue: "VERIFIZIERT",

  featuresTitle: "WAS ES KANN",
  featLibraryTitle: "LIEST DEINE STEAM-BIBLIOTHEK",
  featLibraryText:
    "Native, ~/.steam- und Flatpak-Installationen und jeden Bibliotheksordner, auch auf anderen Laufwerken. Mit deinem eigenen Steam-Web-API-Schlüssel siehst du alle deine Spiele.",
  featHardwareTitle: "KENNT DEINE HARDWARE",
  featHardwareText:
    "CPU, RAM, jede GPU über ihre PCI-ID, Grafikspeicher, Vulkan und Treiber, direkt aus deinem System gelesen. Hybrid-Laptops nutzen die dedizierte GPU.",
  featCatalogTitle: "DURCHSUCHT GANZ STEAM",
  featCatalogText:
    "Jedes Spiel auf Steam, die beliebtesten zuerst. Filtere nach dem, was dein PC schafft, ProtonDB-Stufe, nativem Linux, Deck-Verifizierung oder Anti-Cheat.",
  featWhatIfTitle: "FRAGT „WAS WÄRE, WENN?“",
  featWhatIfText: "Wähle eine andere GPU oder CPU und sieh vor dem Aufrüsten, welche Spiele ihr Urteil ändern.",
  featLinuxTitle: "DENKT LINUX",
  featLinuxText:
    "ProtonDB-Bewertungen, Anti-Cheat, das Linux sperrt, von AreWeAntiCheatYet, der Steam-Deck-Status und die Vulkan-Version, die Proton braucht.",
  featOfflineTitle: "FUNKTIONIERT OFFLINE",
  featOfflineText:
    "Die Spieledatenbank wird einmal geladen und im Hintergrund aktualisiert. Die App sagt dir, wenn es eine neue Version gibt.",
  shotCatalog: "Der Katalog: die beliebtesten Steam-Spiele als Cover mit Stempeln, Suchfeld und Filtern",

  privacyTitle: "NICHTS ÜBER DICH VERLÄSST DEINEN PC",
  privacyLead: "Keine Konten, keine Telemetrie, keine eigenen Server. Nur mit diesen Adressen spricht rigcheck:",
  privacyGithub: "die Spieledatenbank und der Hinweis auf neue Versionen",
  privacySteamStore: "Spiele, die die Datenbank noch nicht geprüft hat",
  privacySteamCdn: "Cover",
  privacySteamApi: "deine Spiele, nur wenn du deinen eigenen Steam-Schlüssel einträgst",
  privacySite: "Auch diese Seite verfolgt dich nicht: keine Analyse, keine Cookies, keine fremden Schriften.",

  installTitle: "INSTALLATION",
  installLead: "Lade das Paket für deine Distribution aus dem neuesten Release.",
  installAppImage: "Jede Distribution",
  installDeb: "Debian, Ubuntu, Mint",
  installRpm: "Fedora, openSUSE",
  installTarball: "Von Hand installieren",
  installGet: "HERUNTERLADEN",
  installVulkan: "Installiere auch vulkan-tools, damit rigcheck die Vulkan-Version und den Grafikspeicher deiner GPU lesen kann.",
  installSoon: "BALD",
  installAur: "rigcheck-bin für Releases, rigcheck-git für den neuesten Code.",
  installFlathub: "Mit einem Klick aus GNOME Software und KDE Discover.",
  installAllReleases: "ALLE RELEASES",

  footerLicense: "MIT-Lizenz",
  footerBy: "Von",
  footerData: "Daten von Steam, ProtonDB und AreWeAntiCheatYet. Nicht mit Valve verbunden.",
};

export const strings: Record<Lang, Strings> = { en, tr, de };
