import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import {
  AlertTriangle,
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

import { defaultLocale, isLocale, locales } from "../../../../i18n/routing";
import {
  getLatestPreviewRelease,
  type GitHubRelease,
  type ReleaseAsset,
} from "../../../../lib/github-release";
import { absoluteUrl, siteName } from "../../../../site";

export const revalidate = 60;

type PreviewDownloadPageProps = {
  params: {
    locale: string;
  };
};

type PreviewTranslator = Awaited<ReturnType<typeof getTranslations>>;

export async function generateMetadata({
  params,
}: PreviewDownloadPageProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "DownloadPreviewPage" });
  const canonical = `/${locale}/download/preview`;
  const languages = Object.fromEntries(
    locales.map((item) => [item, `/${item}/download/preview`]),
  );

  return {
    title: t("metadata.title"),
    description: t("metadata.description"),
    alternates: {
      canonical,
      languages: {
        ...languages,
        "x-default": `/${defaultLocale}/download/preview`,
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

export default async function PreviewDownloadPage({
  params,
}: PreviewDownloadPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({
    locale,
    namespace: "DownloadPreviewPage",
  });
  const releaseState = await getLatestPreviewRelease();

  return (
    <main className="bg-background text-foreground">
      <section className="border-b border-border bg-background">
        <div className="mx-auto grid max-w-6xl gap-8 px-6 py-14 lg:grid-cols-[0.95fr_1.05fr] lg:py-20">
          <div>
            <p className="text-sm font-semibold text-primary">
              {t("hero.eyebrow")}
            </p>
            <h1 className="mt-4 font-display text-5xl font-semibold leading-apple-headline sm:text-6xl">
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
                <a href={`/${locale}/download`}>
                  <Download aria-hidden="true" />
                  {t("hero.stableAction")}
                </a>
              </Button>
              <Button asChild size="lg" variant="secondary">
                <a href="https://github.com/colna/CCUse/releases">
                  <Github aria-hidden="true" />
                  {t("hero.githubAction")}
                </a>
              </Button>
            </nav>
          </div>

          <PreviewSummary locale={locale} state={releaseState} t={t} />
        </div>
      </section>

      <section className="border-b border-border bg-amber-500/5">
        <div className="mx-auto max-w-6xl px-6 py-8">
          <div className="flex gap-4 rounded-lg border border-amber-500/30 bg-background p-5">
            <AlertTriangle
              aria-hidden="true"
              className="mt-0.5 h-5 w-5 shrink-0 text-amber-600 dark:text-amber-300"
            />
            <div>
              <h2 className="font-display text-xl font-semibold">
                {t("risk.title")}
              </h2>
              <p className="mt-2 text-sm leading-6 text-muted-foreground">
                {t("risk.description")}
              </p>
            </div>
          </div>
        </div>
      </section>

      <section className="bg-muted/40" id="preview-assets">
        <div className="mx-auto max-w-6xl px-6 py-16">
          <div className="max-w-2xl">
            <p className="text-sm font-semibold text-primary">
              {t("assets.eyebrow")}
            </p>
            <h2 className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl">
              {t("assets.title")}
            </h2>
            <p className="mt-4 text-base leading-7 text-muted-foreground">
              {t("assets.description")}
            </p>
          </div>

          {releaseState.status === "ready" ? (
            <PreviewAssetList
              assets={releaseState.release.assets}
              locale={locale}
              t={t}
            />
          ) : (
            <UnavailableState reason={releaseState.reason} t={t} />
          )}
        </div>
      </section>
    </main>
  );
}

function PreviewSummary({
  locale,
  state,
  t,
}: {
  locale: string;
  state: Awaited<ReturnType<typeof getLatestPreviewRelease>>;
  t: PreviewTranslator;
}) {
  if (state.status === "unavailable") {
    return <UnavailableState reason={state.reason} t={t} />;
  }

  const { release } = state;

  return (
    <Card className="border-border/80 bg-card shadow-none">
      <CardContent className="p-6">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-amber-500/10 text-amber-600 dark:text-amber-300">
            <PackageCheck aria-hidden="true" className="h-6 w-6" />
          </div>
          <span className="rounded-md bg-amber-500/10 px-2.5 py-1 text-xs font-semibold text-amber-700 dark:text-amber-200">
            {t("release.prerelease")}
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
            value={t("release.assetCount", { count: release.assetCount })}
          />
          <ReleaseMetric
            Icon={CalendarClock}
            label={t("release.published")}
            value={formatDate(release.publishedAt, locale)}
          />
        </dl>
        <a
          className="mt-6 inline-flex items-center gap-2 text-sm font-semibold text-primary transition-colors hover:text-primary/80 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
          href={release.htmlUrl}
        >
          {t("release.viewRelease")}
          <ExternalLink aria-hidden="true" className="h-4 w-4" />
        </a>
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

function PreviewAssetList({
  assets,
  locale,
  t,
}: {
  assets: GitHubRelease["assets"];
  locale: string;
  t: PreviewTranslator;
}) {
  if (assets.length === 0) {
    return <UnavailableState reason={t("assets.empty")} t={t} />;
  }

  return (
    <ul className="mt-10 grid gap-4 lg:grid-cols-3">
      {assets.map((asset) => (
        <li key={asset.id}>
          <PreviewAssetCard asset={asset} locale={locale} t={t} />
        </li>
      ))}
    </ul>
  );
}

function PreviewAssetCard({
  asset,
  locale,
  t,
}: {
  asset: ReleaseAsset;
  locale: string;
  t: PreviewTranslator;
}) {
  return (
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
  );
}

function UnavailableState({
  reason,
  t,
}: {
  reason: string;
  t: PreviewTranslator;
}) {
  return (
    <Card className="border-border/80 bg-card shadow-none">
      <CardContent className="p-6">
        <h2 className="font-display text-xl font-semibold">
          {t("unavailable.title")}
        </h2>
        <p className="mt-3 text-sm leading-6 text-muted-foreground">
          {t("unavailable.description")}
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
