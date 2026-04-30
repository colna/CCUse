export const platformRecommendationIds = [
  "macos-aarch64",
  "macos-x64",
  "windows-x64",
] as const;

export type PlatformRecommendationId =
  | (typeof platformRecommendationIds)[number]
  | "unknown";

export type PlatformDetectionHints = {
  architecture?: string;
  platform?: string;
  userAgent: string;
  userAgentDataPlatform?: string;
};

export type DownloadAssetCandidate = {
  downloadUrl: string;
  name: string;
  sizeLabel: string;
};

export function detectPlatformFromUserAgent({
  architecture = "",
  platform = "",
  userAgent,
  userAgentDataPlatform = "",
}: PlatformDetectionHints): PlatformRecommendationId {
  const combined =
    `${userAgent} ${platform} ${userAgentDataPlatform}`.toLowerCase();
  const normalizedArchitecture = architecture.toLowerCase();

  if (combined.includes("windows")) {
    return "windows-x64";
  }

  if (
    combined.includes("mac os") ||
    combined.includes("macintosh") ||
    combined.includes("macintel") ||
    combined.includes("macos")
  ) {
    if (
      normalizedArchitecture.includes("arm") ||
      normalizedArchitecture.includes("aarch64") ||
      combined.includes("arm64") ||
      combined.includes("aarch64")
    ) {
      return "macos-aarch64";
    }

    if (
      normalizedArchitecture.includes("x86") ||
      normalizedArchitecture.includes("x64") ||
      normalizedArchitecture.includes("amd64") ||
      combined.includes("x86_64")
    ) {
      return "macos-x64";
    }
  }

  return "unknown";
}

export function findRecommendedAsset(
  assets: DownloadAssetCandidate[],
  platformId: PlatformRecommendationId,
) {
  if (platformId === "unknown") {
    return undefined;
  }

  return assets.find((asset) => matchesAssetName(asset.name, platformId));
}

export function matchesAssetName(
  name: string,
  platformId: Exclude<PlatformRecommendationId, "unknown">,
) {
  const normalizedName = name.toLowerCase();

  if (platformId === "macos-aarch64") {
    return normalizedName.endsWith("_aarch64.dmg");
  }

  if (platformId === "macos-x64") {
    return (
      normalizedName.endsWith("_x64.dmg") &&
      !normalizedName.endsWith("_x64-setup.exe")
    );
  }

  return normalizedName.endsWith("_x64-setup.exe");
}
