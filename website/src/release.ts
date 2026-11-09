// The latest app release, read from GitHub once per build, so the download
// buttons name the version and link straight to its files. If GitHub can't
// be reached the buttons fall back to the releases page.

const repo = "https://github.com/Glory42/rigcheck";
export const releasesUrl = `${repo}/releases`;
export const repoUrl = repo;

export type Release = {
  version: string;
  appImage: string;
  deb: string;
  rpm: string;
  tarball: string;
};

let latest: Promise<Release | null> | undefined;

export function latestRelease(): Promise<Release | null> {
  latest ??= fetchLatest();
  return latest;
}

async function fetchLatest(): Promise<Release | null> {
  try {
    const response = await fetch("https://api.github.com/repos/Glory42/rigcheck/releases/latest", {
      headers: { Accept: "application/vnd.github+json", "User-Agent": "rigcheck-website" },
    });
    if (!response.ok) throw new Error(`GitHub answered ${response.status}`);
    const { tag_name: tag } = (await response.json()) as { tag_name: string };
    const version = tag.replace(/^v/, "");
    // The names release-app.yml gives each package.
    const files = `${repo}/releases/download/${tag}`;
    return {
      version,
      appImage: `${files}/rigcheck-${version}-x86_64.AppImage`,
      deb: `${files}/rigcheck_${version}-1_amd64.deb`,
      rpm: `${files}/rigcheck-${version}-1.x86_64.rpm`,
      tarball: `${files}/rigcheck-${version}-x86_64-linux.tar.gz`,
    };
  } catch (error) {
    console.warn(`No latest release, linking the releases page instead: ${error}`);
    return null;
  }
}
