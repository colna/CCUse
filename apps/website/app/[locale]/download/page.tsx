import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import {
  ArrowRight,
  CalendarClock,
  Download,
  ExternalLink,
  FileArchive,
  Github,
  PackageCheck,
} from "lucide-react";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { defaultLocale, isLocale, locales } from "../../../i18n/routing";
import { absoluteUrl, siteName } from "../../../site";

export const revalidate = 60;

const latestReleaseApiUrl =
  "https://api.github.com/repos/colna/CCUse/releases/latest";

type DownloadPageProps = {
  params: {
    locale: string;
  };
};

type ReleaseAsset = {
  id: number;
  name: string;
  size: number;
  contentType: string;
  downloadUrl: string;
  updatedAt: string;
};

type LatestRelease = {
  tagName: string;
  name: string;
  htmlUrl: string;
  publishedAt: string;
  prerelease: boolean;
  assets: ReleaseAsset[];
};

type ReleaseState =
  | {
      status: "ready";
      release: LatestRelease;
    }
  | {
      status: "unavailable";
      reason: string;
    };

type DownloadTranslator = Awaited<ReturnType<typeof getTranslations>>;

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
  const releaseState = await getLatestRelease();

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
                <a href="#release-assets">
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
            </nav>
          </div>

          <ReleaseSummary locale={locale} state={releaseState} t={t} />
        </div>
      </section>

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

async function getLatestRelease(): Promise<ReleaseState> {
  try {
    const response = await fetch(latestReleaseApiUrl, {
      headers: {
        Accept: "application/vnd.github+json",
        "User-Agent": "CCUse website",
        "X-GitHub-Api-Version": "2022-11-28",
      },
      next: { revalidate },
    });

    if (!response.ok) {
      return {
        status: "unavailable",
        reason: `GitHub API returned ${response.status}`,
      };
    }

    const release = normalizeRelease(await response.json());

    if (!release) {
      return {
        status: "unavailable",
        reason: "GitHub API response was missing release fields",
      };
    }

    return { status: "ready", release };
  } catch (error) {
    return {
      status: "unavailable",
      reason: error instanceof Error ? error.message : "Unknown fetch error",
    };
  }
}

function normalizeRelease(value: unknown): LatestRelease | null {
  if (!isRecord(value)) {
    return null;
  }

  const tagName = readString(value, "tag_name");
  const htmlUrl = readString(value, "html_url");
  const publishedAt = readString(value, "published_at");

  if (!tagName || !htmlUrl || !publishedAt) {
    return null;
  }

  const assets = Array.isArray(value.assets)
    ? value.assets.flatMap((asset) => {
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
            id,
            name,
            size,
            contentType,
            downloadUrl,
            updatedAt,
          },
        ];
      })
    : [];

  return {
    tagName,
    name: readString(value, "name") || tagName,
    htmlUrl,
    publishedAt,
    prerelease: readBoolean(value, "prerelease"),
    assets,
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
              count: release.assets.length,
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
  release: LatestRelease;
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
