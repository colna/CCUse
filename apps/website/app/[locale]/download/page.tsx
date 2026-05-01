import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import {
  ArrowRight,
  CalendarClock,
  Download,
  ExternalLink,
  FileArchive,
  Github,
  Laptop as LaptopIcon,
  MonitorDown,
  PackageCheck,
} from "lucide-react";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { DownloadPlatformRecommendation } from "../../../components/download-platform-recommendation";
import { defaultLocale, isLocale, locales } from "../../../i18n/routing";
import {
  matchesAssetName,
  type DownloadAssetCandidate,
  type PlatformRecommendationId,
} from "../../../lib/download-platform";
import {
  getLatestStableRelease,
  type GitHubRelease,
  type ReleaseAsset,
  type ReleaseState,
} from "../../../lib/github-release";
import { absoluteUrl, siteName } from "../../../site";

export const revalidate = 60;

const downloadTargets = [
  {
    key: "macosAarch64",
    platformId: "macos-aarch64",
    Icon: LaptopIcon,
  },
  {
    key: "macosX64",
    platformId: "macos-x64",
    Icon: LaptopIcon,
  },
  {
    key: "windowsX64",
    platformId: "windows-x64",
    Icon: MonitorDown,
  },
] as const;

type DownloadPageProps = {
  params: {
    locale: string;
  };
};

type DownloadTranslator = Awaited<ReturnType<typeof getTranslations>>;
type DownloadTarget = (typeof downloadTargets)[number];

export async function generateMetadata({
  params,
}: DownloadPageProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "DownloadPage" });
  const canonical = `/${locale}/download`;
  const languages = Object.fromEntries(
    locales.map((item) => [item, `/${item}/download`]),
  );

  return {
    title: t("metadata.title"),
    description: t("metadata.description"),
    alternates: {
      canonical,
      languages: {
        ...languages,
        "x-default": `/${defaultLocale}/download`,
      },
    },
    openGraph: {
      title: `${t("metadata.title")} | ${siteName}`,
      description: t("metadata.description"),
      url: absoluteUrl(canonical),
      siteName,
      type: "website",
      locale: locale === "zh" ? "zh_CN" : "en_US",
      alternateLocale: locale === "zh" ? ["en_US"] : ["zh_CN"],
      images: [
        {
          url: absoluteUrl("/opengraph-image.png"),
          width: 801,
          height: 801,
          alt: siteName,
        },
      ],
    },
    twitter: {
      card: "summary",
      title: `${t("metadata.title")} | ${siteName}`,
      description: t("metadata.description"),
      images: [absoluteUrl("/opengraph-image.png")],
    },
  };
}

export default async function DownloadPage({ params }: DownloadPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({ locale, namespace: "DownloadPage" });
  const releaseState = await getLatestStableRelease();
  const assetCandidates =
    releaseState.status === "ready"
      ? releaseState.release.assets.map(toAssetCandidate)
      : [];

  return (
    <main className="bg-background text-foreground">
      <section
        aria-labelledby="download-title"
        className="overflow-hidden border-b border-border bg-background"
      >
        <div className="mx-auto grid max-w-6xl items-center gap-10 px-6 py-14 sm:py-16 lg:grid-cols-[0.92fr_1.08fr] lg:py-20">
          <div>
            <p className="text-sm font-semibold text-primary">
              {t("hero.eyebrow")}
            </p>
            <h1
              className="mt-4 font-display text-5xl font-semibold leading-apple-headline sm:text-6xl"
              id="download-title"
            >
              {t("hero.title")}
            </h1>
            <p className="mt-5 max-w-2xl text-base leading-7 text-muted-foreground sm:text-lg sm:leading-8">
              {t("hero.description")}
            </p>
            <nav
              aria-label={t("hero.actionsLabel")}
              className="mt-8 flex flex-col gap-3 sm:flex-row"
            >
              <Button asChild size="lg">
                <a href="#download-packages">
                  <Download aria-hidden="true" />
                  {t("hero.primaryAction")}
                </a>
              </Button>
              <Button asChild size="lg" variant="secondary">
                <a href="https://github.com/colna/CCUse/releases">
                  <Github aria-hidden="true" />
                  {t("hero.secondaryAction")}
                </a>
              </Button>
              <Button asChild size="lg" variant="outline">
                <a href={`/${locale}/download/preview`}>
                  <PackageCheck aria-hidden="true" />
                  {t("hero.previewAction")}
                </a>
              </Button>
            </nav>
          </div>

          <ReleaseSummary locale={locale} state={releaseState} t={t} />
        </div>
      </section>

      <section className="border-b border-border bg-background">
        <div className="mx-auto max-w-6xl px-6 py-12">
          <DownloadPlatformRecommendation
            assets={assetCandidates}
            labels={getPlatformLabels(t)}
          />
        </div>
      </section>

      <DownloadPackagesSection
        release={releaseState.status === "ready" ? releaseState.release : null}
        t={t}
      />

      <section
        aria-labelledby="release-assets-title"
        className="bg-muted/40"
        id="release-assets"
      >
        <div className="mx-auto max-w-6xl px-6 py-16">
          <div className="flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
            <div className="max-w-2xl">
              <p className="text-sm font-semibold text-primary">
                {t("assets.eyebrow")}
              </p>
              <h2
                className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl"
                id="release-assets-title"
              >
                {t("assets.title")}
              </h2>
              <p className="mt-4 text-base leading-7 text-muted-foreground">
                {t("assets.description")}
              </p>
            </div>

            {releaseState.status === "ready" ? (
              <a
                className="inline-flex items-center gap-2 text-sm font-semibold text-primary transition-colors hover:text-primary/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                href={releaseState.release.htmlUrl}
              >
                {t("assets.viewRelease")}
                <ExternalLink aria-hidden="true" className="h-4 w-4" />
              </a>
            ) : null}
          </div>

          {releaseState.status === "ready" ? (
            <AssetList locale={locale} release={releaseState.release} t={t} />
          ) : (
            <UnavailableState reason={releaseState.reason} t={t} />
          )}
        </div>
      </section>
    </main>
  );
}

function DownloadPackagesSection({
  release,
  t,
}: {
  release: GitHubRelease | null;
  t: DownloadTranslator;
}) {
  return (
    <section
      aria-labelledby="download-packages-title"
      className="border-b border-border bg-background"
      id="download-packages"
    >
      <div className="mx-auto max-w-6xl px-6 py-16">
        <div className="max-w-2xl">
          <p className="text-sm font-semibold text-primary">
            {t("packages.eyebrow")}
          </p>
          <h2
            className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl"
            id="download-packages-title"
          >
            {t("packages.title")}
          </h2>
          <p className="mt-4 text-base leading-7 text-muted-foreground">
            {t("packages.description")}
          </p>
        </div>

        <ul className="mt-10 grid gap-4 lg:grid-cols-3">
          {downloadTargets.map((target) => (
            <li key={target.key}>
              <DownloadPackageCard
                asset={findTargetAsset(release, target.platformId)}
                target={target}
                t={t}
              />
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}

function DownloadPackageCard({
  asset,
  target,
  t,
}: {
  asset?: ReleaseAsset;
  target: DownloadTarget;
  t: DownloadTranslator;
}) {
  const { Icon, key } = target;

  return (
    <Card className="h-full border-border/80 bg-card shadow-none">
      <CardContent className="flex h-full flex-col p-6">
        <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon aria-hidden="true" className="h-6 w-6" />
        </div>
        <h3 className="mt-5 font-display text-xl font-semibold leading-7">
          {t(`packages.targets.${key}.title`)}
        </h3>
        <p className="mt-3 text-sm leading-6 text-muted-foreground">
          {t(`packages.targets.${key}.description`)}
        </p>
        <dl className="mt-5 grid gap-3">
          <div className="rounded-lg border border-border bg-background p-3">
            <dt className="text-xs font-medium text-muted-foreground">
              {t("packages.filenamePattern")}
            </dt>
            <dd className="mt-1 font-mono text-sm font-semibold">
              {t(`packages.targets.${key}.pattern`)}
            </dd>
          </div>
          <div className="rounded-lg border border-border bg-background p-3">
            <dt className="text-xs font-medium text-muted-foreground">
              {asset ? t("packages.size") : t("packages.status")}
            </dt>
            <dd className="mt-1 text-sm font-semibold">
              {asset ? formatBytes(asset.size) : t("packages.missing")}
            </dd>
          </div>
          {asset ? (
            <div className="rounded-lg border border-border bg-background p-3">
              <dt className="text-xs font-medium text-muted-foreground">
                {t("packages.sha256")}
              </dt>
              <dd className="mt-1 break-all font-mono text-xs font-semibold leading-5">
                {asset.sha256 ?? t("packages.sha256Missing")}
              </dd>
            </div>
          ) : null}
        </dl>

        {asset ? (
          <div className="mt-auto pt-6">
            <Button asChild className="w-full">
              <a href={asset.downloadUrl}>
                <Download aria-hidden="true" />
                {t("packages.download")}
              </a>
            </Button>
          </div>
        ) : (
          <p className="mt-auto pt-6 text-sm leading-6 text-muted-foreground">
            {t("packages.missingDescription")}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

function findTargetAsset(
  release: GitHubRelease | null,
  platformId: Exclude<PlatformRecommendationId, "unknown">,
) {
  return release?.assets.find((asset) =>
    matchesAssetName(asset.name, platformId),
  );
}

function toAssetCandidate(asset: ReleaseAsset): DownloadAssetCandidate {
  return {
    downloadUrl: asset.downloadUrl,
    name: asset.name,
    sizeLabel: formatBytes(asset.size),
  };
}

function getPlatformLabels(t: DownloadTranslator) {
  return {
    detectedLabel: t("platform.detectedLabel"),
    description: t("platform.description"),
    downloadRecommended: t("platform.downloadRecommended"),
    noAsset: t("platform.noAsset"),
    options: {
      "macos-aarch64": {
        description: t("platform.options.macosAarch64.description"),
        title: t("platform.options.macosAarch64.title"),
      },
      "macos-x64": {
        description: t("platform.options.macosX64.description"),
        title: t("platform.options.macosX64.title"),
      },
      "windows-x64": {
        description: t("platform.options.windowsX64.description"),
        title: t("platform.options.windowsX64.title"),
      },
    },
    title: t("platform.title"),
    unknownDescription: t("platform.unknownDescription"),
    unknownTitle: t("platform.unknownTitle"),
  };
}

function ReleaseSummary({
  locale,
  state,
  t,
}: {
  locale: string;
  state: ReleaseState;
  t: DownloadTranslator;
}) {
  if (state.status === "unavailable") {
    return (
      <Card className="border-border/80 bg-card shadow-none">
        <CardContent className="p-6">
          <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-amber-500/10 text-amber-600 dark:text-amber-300">
            <PackageCheck aria-hidden="true" className="h-6 w-6" />
          </div>
          <h2 className="mt-6 font-display text-2xl font-semibold">
            {t("release.unavailableTitle")}
          </h2>
          <p className="mt-3 text-sm leading-6 text-muted-foreground">
            {t("release.unavailableDescription")}
          </p>
          <p className="mt-4 rounded-lg border border-border bg-muted/40 p-3 font-mono text-xs text-muted-foreground">
            {state.reason}
          </p>
        </CardContent>
      </Card>
    );
  }

  const { release } = state;

  return (
    <Card className="overflow-hidden border-border/80 bg-card shadow-none">
      <CardContent className="p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-primary/10 text-primary">
            <PackageCheck aria-hidden="true" className="h-6 w-6" />
          </div>
          <span className="rounded-md bg-emerald-500/10 px-2.5 py-1 text-xs font-semibold text-emerald-700 dark:text-emerald-200">
            {release.prerelease ? t("release.prerelease") : t("release.stable")}
          </span>
        </div>
        <p className="mt-6 text-sm font-semibold text-primary">
          {t("release.latest")}
        </p>
        <h2 className="mt-2 font-display text-3xl font-semibold leading-apple-tile">
          {release.name}
        </h2>
        <dl className="mt-6 grid gap-3 sm:grid-cols-3">
          <ReleaseMetric
            Icon={FileArchive}
            label={t("release.version")}
            value={release.tagName}
          />
          <ReleaseMetric
            Icon={Download}
            label={t("release.assets")}
            value={t("release.assetCount", {
              count: release.assetCount,
            })}
          />
          <ReleaseMetric
            Icon={CalendarClock}
            label={t("release.published")}
            value={formatDate(release.publishedAt, locale)}
          />
        </dl>
      </CardContent>
    </Card>
  );
}

function ReleaseMetric({
  Icon,
  label,
  value,
}: {
  Icon: typeof FileArchive;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-lg border border-border bg-background p-4">
      <dt className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
        <Icon aria-hidden="true" className="h-4 w-4 text-primary" />
        {label}
      </dt>
      <dd className="mt-2 text-sm font-semibold leading-5">{value}</dd>
    </div>
  );
}

function AssetList({
  locale,
  release,
  t,
}: {
  locale: string;
  release: GitHubRelease;
  t: DownloadTranslator;
}) {
  if (release.assets.length === 0) {
    return (
      <Card className="mt-10 border-border/80 bg-card shadow-none">
        <CardContent className="p-6">
          <p className="text-sm leading-6 text-muted-foreground">
            {t("assets.empty")}
          </p>
        </CardContent>
      </Card>
    );
  }

  return (
    <ul className="mt-10 grid gap-4 lg:grid-cols-3">
      {release.assets.map((asset) => (
        <li key={asset.id}>
          <Card className="h-full border-border/80 bg-card shadow-none">
            <CardContent className="flex h-full flex-col p-6">
              <div className="flex items-start justify-between gap-4">
                <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <FileArchive aria-hidden="true" className="h-5 w-5" />
                </div>
                <span className="rounded-md bg-muted px-2.5 py-1 text-xs font-semibold text-muted-foreground">
                  {formatBytes(asset.size)}
                </span>
              </div>
              <h3 className="mt-5 break-words font-display text-xl font-semibold leading-7">
                {asset.name}
              </h3>
              <dl className="mt-5 grid gap-3 text-sm">
                <div>
                  <dt className="text-xs font-medium text-muted-foreground">
                    {t("assets.contentType")}
                  </dt>
                  <dd className="mt-1 font-medium">{asset.contentType}</dd>
                </div>
                <div>
                  <dt className="text-xs font-medium text-muted-foreground">
                    {t("assets.updated")}
                  </dt>
                  <dd className="mt-1 font-medium">
                    {formatDate(asset.updatedAt, locale)}
                  </dd>
                </div>
                <div>
                  <dt className="text-xs font-medium text-muted-foreground">
                    {t("assets.sha256")}
                  </dt>
                  <dd className="mt-1 break-all font-mono text-xs font-medium leading-5">
                    {asset.sha256 ?? t("assets.sha256Missing")}
                  </dd>
                </div>
              </dl>
              <a
                className="mt-auto inline-flex items-center gap-2 pt-6 text-sm font-semibold text-primary transition-colors hover:text-primary/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                href={asset.downloadUrl}
              >
                {t("assets.downloadAsset")}
                <ArrowRight aria-hidden="true" className="h-4 w-4" />
              </a>
            </CardContent>
          </Card>
        </li>
      ))}
    </ul>
  );
}

function UnavailableState({
  reason,
  t,
}: {
  reason: string;
  t: DownloadTranslator;
}) {
  return (
    <Card className="mt-10 border-border/80 bg-card shadow-none">
      <CardContent className="p-6">
        <h3 className="font-display text-xl font-semibold">
          {t("assets.unavailableTitle")}
        </h3>
        <p className="mt-3 text-sm leading-6 text-muted-foreground">
          {t("assets.unavailableDescription")}
        </p>
        <p className="mt-4 rounded-lg border border-border bg-muted/40 p-3 font-mono text-xs text-muted-foreground">
          {reason}
        </p>
      </CardContent>
    </Card>
  );
}

function formatDate(value: string, locale: string) {
  const date = new Date(value);

  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function formatBytes(value: number) {
  if (value < 1024) {
    return `${value} B`;
  }

  if (value < 1024 * 1024) {
    return `${(value / 1024).toFixed(1)} KB`;
  }

  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}
