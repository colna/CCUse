export const releaseRevalidate = 60;

const githubApiBaseUrl = "https://api.github.com/repos/colna/CCUse";
const latestReleaseApiUrl = `${githubApiBaseUrl}/releases/latest`;
const releasesApiUrl = `${githubApiBaseUrl}/releases?per_page=20`;
const checksumSuffixes = [".sha256", ".sha256.txt"] as const;
const checksumAssetNames = new Set([
  "checksums.txt",
  "sha256sums.txt",
  "sha256sum.txt",
]);

export type ReleaseAsset = {
  checksumAssetName: string | null;
  checksumUrl: string | null;
  contentType: string;
  downloadUrl: string;
  id: number;
  name: string;
  sha256: string | null;
  size: number;
  updatedAt: string;
};

export type ReleaseChecksumAsset = {
  contentType: string;
  downloadUrl: string;
  id: number;
  name: string;
  size: number;
  updatedAt: string;
};

export type GitHubRelease = {
  assetCount: number;
  assets: ReleaseAsset[];
  checksumAssets: ReleaseChecksumAsset[];
  htmlUrl: string;
  name: string;
  prerelease: boolean;
  publishedAt: string;
  tagName: string;
};

export type ReleaseState =
  | {
      release: GitHubRelease;
      status: "ready";
    }
  | {
      reason: string;
      status: "unavailable";
    };

type RawReleaseAsset = Omit<
  ReleaseAsset,
  "checksumAssetName" | "checksumUrl" | "sha256"
>;

type ChecksumRecord = {
  checksumAssetName: string;
  checksumUrl: string;
  sha256: string;
};

export async function getLatestStableRelease(): Promise<ReleaseState> {
  return getReleaseFromUrl(latestReleaseApiUrl);
}

export async function getLatestPreviewRelease(): Promise<ReleaseState> {
  try {
    const response = await fetch(releasesApiUrl, {
      headers: githubHeaders,
      next: { revalidate: releaseRevalidate },
    });

    if (!response.ok) {
      return {
        reason: `GitHub API returned ${response.status}`,
        status: "unavailable",
      };
    }

    const value = await response.json();

    if (!Array.isArray(value)) {
      return {
        reason: "GitHub API response was not a release list",
        status: "unavailable",
      };
    }

    const previewRelease = value.find((item) => {
      if (!isRecord(item)) {
        return false;
      }

      return (
        readBoolean(item, "prerelease") &&
        /^v?0\./.test(readString(item, "tag_name"))
      );
    });

    if (!previewRelease) {
      return {
        reason: "No 0.x pre-release was found in the latest GitHub releases",
        status: "unavailable",
      };
    }

    return normalizeReleaseState(previewRelease);
  } catch (error) {
    return {
      reason: error instanceof Error ? error.message : "Unknown fetch error",
      status: "unavailable",
    };
  }
}

async function getReleaseFromUrl(url: string): Promise<ReleaseState> {
  try {
    const response = await fetch(url, {
      headers: githubHeaders,
      next: { revalidate: releaseRevalidate },
    });

    if (!response.ok) {
      return {
        reason: `GitHub API returned ${response.status}`,
        status: "unavailable",
      };
    }

    return normalizeReleaseState(await response.json());
  } catch (error) {
    return {
      reason: error instanceof Error ? error.message : "Unknown fetch error",
      status: "unavailable",
    };
  }
}

const githubHeaders = {
  Accept: "application/vnd.github+json",
  "User-Agent": "CCUse website",
  "X-GitHub-Api-Version": "2022-11-28",
};

async function normalizeReleaseState(value: unknown): Promise<ReleaseState> {
  const release = await normalizeRelease(value);

  if (!release) {
    return {
      reason: "GitHub API response was missing release fields",
      status: "unavailable",
    };
  }

  return { release, status: "ready" };
}

async function normalizeRelease(value: unknown): Promise<GitHubRelease | null> {
  if (!isRecord(value)) {
    return null;
  }

  const tagName = readString(value, "tag_name");
  const htmlUrl = readString(value, "html_url");
  const publishedAt = readString(value, "published_at");

  if (!tagName || !htmlUrl || !publishedAt) {
    return null;
  }

  const rawAssets = Array.isArray(value.assets)
    ? value.assets.flatMap(normalizeAsset)
    : [];
  const binaryAssets = rawAssets.filter(
    (asset) => !isChecksumAssetName(asset.name),
  );
  const checksumAssets = rawAssets.filter((asset) =>
    isChecksumAssetName(asset.name),
  );
  const checksumRecords = await readChecksumRecords(
    checksumAssets,
    binaryAssets,
  );
  const assets = binaryAssets.map((asset) => {
    const checksum = checksumRecords.get(asset.name.toLowerCase());

    return {
      ...asset,
      checksumAssetName: checksum?.checksumAssetName ?? null,
      checksumUrl: checksum?.checksumUrl ?? null,
      sha256: checksum?.sha256 ?? null,
    };
  });

  return {
    assetCount: rawAssets.length,
    assets,
    checksumAssets,
    htmlUrl,
    name: readString(value, "name") || tagName,
    prerelease: readBoolean(value, "prerelease"),
    publishedAt,
    tagName,
  };
}

function normalizeAsset(asset: unknown): RawReleaseAsset[] {
  if (!isRecord(asset)) {
    return [];
  }

  const id = readNumber(asset, "id");
  const name = readString(asset, "name");
  const size = readNumber(asset, "size");
  const contentType = readString(asset, "content_type");
  const downloadUrl = readString(asset, "browser_download_url");
  const updatedAt = readString(asset, "updated_at");

  if (
    id === null ||
    !name ||
    size === null ||
    !contentType ||
    !downloadUrl ||
    !updatedAt
  ) {
    return [];
  }

  return [
    {
      contentType,
      downloadUrl,
      id,
      name,
      size,
      updatedAt,
    },
  ];
}

async function readChecksumRecords(
  checksumAssets: RawReleaseAsset[],
  binaryAssets: RawReleaseAsset[],
) {
  const checksumEntries = await Promise.all(
    checksumAssets.map(async (asset) => {
      try {
        const response = await fetch(asset.downloadUrl, {
          headers: { "User-Agent": "CCUse website" },
          next: { revalidate: releaseRevalidate },
        });

        if (!response.ok) {
          return [];
        }

        return parseChecksumText(asset, await response.text(), binaryAssets);
      } catch {
        return [];
      }
    }),
  );

  return checksumEntries.flat().reduce((checksums, record) => {
    checksums.set(record[0].toLowerCase(), record[1]);
    return checksums;
  }, new Map<string, ChecksumRecord>());
}

function parseChecksumText(
  checksumAsset: RawReleaseAsset,
  text: string,
  binaryAssets: RawReleaseAsset[],
): Array<[string, ChecksumRecord]> {
  const fallbackTargetName = inferChecksumTargetName(checksumAsset.name);
  const records: Array<[string, ChecksumRecord]> = [];

  for (const line of text.split(/\r?\n/)) {
    const hashMatch = line.match(/\b[a-fA-F0-9]{64}\b/);

    if (!hashMatch) {
      continue;
    }

    const matchedAssetName =
      binaryAssets.find((asset) => line.includes(asset.name))?.name ??
      fallbackTargetName;

    if (!matchedAssetName) {
      continue;
    }

    records.push([
      matchedAssetName,
      {
        checksumAssetName: checksumAsset.name,
        checksumUrl: checksumAsset.downloadUrl,
        sha256: hashMatch[0].toLowerCase(),
      },
    ]);
  }

  return records;
}

function inferChecksumTargetName(name: string) {
  const normalizedName = name.toLowerCase();
  const suffix = checksumSuffixes.find((item) => normalizedName.endsWith(item));

  if (!suffix) {
    return "";
  }

  return name.slice(0, -suffix.length);
}

function isChecksumAssetName(name: string) {
  const normalizedName = name.toLowerCase();

  return (
    checksumAssetNames.has(normalizedName) ||
    checksumSuffixes.some((suffix) => normalizedName.endsWith(suffix)) ||
    normalizedName.includes("checksum")
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function readString(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "string" ? value : "";
}

function readNumber(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "number" ? value : null;
}

function readBoolean(record: Record<string, unknown>, key: string) {
  const value = record[key];
  return typeof value === "boolean" ? value : false;
}
