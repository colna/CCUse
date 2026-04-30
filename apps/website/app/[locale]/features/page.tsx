import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import {
  ArrowRight,
  BadgeCheck,
  BarChart3,
  BrainCircuit,
  Download,
  Gauge,
  HeartPulse,
  Laptop,
  Network,
  ServerCog,
  ShieldCheck,
  Shuffle,
  type LucideIcon,
} from "lucide-react";
import type { Metadata } from "next";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { defaultLocale, isLocale, locales } from "../../../i18n/routing";
import { absoluteUrl, siteName } from "../../../site";

const featureSections = [
  {
    key: "failover",
    Icon: Shuffle,
    tone: {
      icon: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-300",
      accent: "bg-emerald-500",
      soft: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-200",
    },
  },
  {
    key: "multiProvider",
    Icon: Network,
    tone: {
      icon: "bg-sky-500/10 text-sky-600 dark:text-sky-300",
      accent: "bg-sky-500",
      soft: "bg-sky-500/10 text-sky-700 dark:text-sky-200",
    },
  },
  {
    key: "healthCheck",
    Icon: HeartPulse,
    tone: {
      icon: "bg-rose-500/10 text-rose-600 dark:text-rose-300",
      accent: "bg-rose-500",
      soft: "bg-rose-500/10 text-rose-700 dark:text-rose-200",
    },
  },
  {
    key: "smartStrategy",
    Icon: BrainCircuit,
    tone: {
      icon: "bg-violet-500/10 text-violet-600 dark:text-violet-300",
      accent: "bg-violet-500",
      soft: "bg-violet-500/10 text-violet-700 dark:text-violet-200",
    },
  },
  {
    key: "monitoring",
    Icon: BarChart3,
    tone: {
      icon: "bg-amber-500/10 text-amber-600 dark:text-amber-300",
      accent: "bg-amber-500",
      soft: "bg-amber-500/10 text-amber-700 dark:text-amber-200",
    },
  },
  {
    key: "crossPlatform",
    Icon: Laptop,
    tone: {
      icon: "bg-zinc-500/10 text-zinc-700 dark:text-zinc-200",
      accent: "bg-zinc-500",
      soft: "bg-zinc-500/10 text-zinc-700 dark:text-zinc-200",
    },
  },
] as const;

const screenshotKeys = ["setup", "routing", "result"] as const;
const detailKeys = ["first", "second", "third"] as const;
const heroFlowKeys = ["client", "proxy", "provider"] as const;

type FeaturesPageProps = {
  params: {
    locale: string;
  };
};

type FeatureKey = (typeof featureSections)[number]["key"];
type ScreenshotKey = (typeof screenshotKeys)[number];
type FeaturesTranslator = Awaited<ReturnType<typeof getTranslations>>;

export async function generateMetadata({
  params,
}: FeaturesPageProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "FeaturesPage" });
  const canonical = `/${locale}/features`;
  const languages = Object.fromEntries(
    locales.map((item) => [item, `/${item}/features`]),
  );

  return {
    title: t("metadata.title"),
    description: t("metadata.description"),
    alternates: {
      canonical,
      languages: {
        ...languages,
        "x-default": `/${defaultLocale}/features`,
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

export default async function FeaturesPage({ params }: FeaturesPageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({ locale, namespace: "FeaturesPage" });

  return (
    <main className="bg-background text-foreground">
      <section
        aria-labelledby="features-hero-title"
        className="overflow-hidden border-b border-border bg-background"
      >
        <div className="mx-auto grid max-w-6xl items-center gap-12 px-6 py-14 sm:py-16 lg:grid-cols-[0.9fr_1.1fr] lg:py-20">
          <div>
            <p className="text-sm font-semibold text-primary">
              {t("hero.eyebrow")}
            </p>
            <h1
              className="mt-4 font-display text-5xl font-semibold leading-apple-headline sm:text-6xl"
              id="features-hero-title"
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
                <a href={`/${locale}/download`}>
                  <Download aria-hidden="true" />
                  {t("hero.primaryAction")}
                </a>
              </Button>
              <Button asChild size="lg" variant="secondary">
                <a href={`/${locale}/docs/getting-started`}>
                  <ServerCog aria-hidden="true" />
                  {t("hero.secondaryAction")}
                </a>
              </Button>
            </nav>
          </div>

          <FeatureRoutePreview t={t} />
        </div>
      </section>

      <section
        aria-labelledby="features-overview-title"
        className="border-b border-border bg-muted/40"
      >
        <div className="mx-auto max-w-6xl px-6 py-12">
          <div className="max-w-2xl">
            <p className="text-sm font-semibold text-primary">
              {t("overview.eyebrow")}
            </p>
            <h2
              className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl"
              id="features-overview-title"
            >
              {t("overview.title")}
            </h2>
          </div>

          <nav
            aria-label={t("overview.navLabel")}
            className="mt-8 grid gap-3 sm:grid-cols-2 lg:grid-cols-3"
          >
            {featureSections.map(({ key, Icon, tone }) => (
              <a
                className="group rounded-lg border border-border bg-card p-4 text-card-foreground transition-colors hover:border-primary/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                href={`#${key}`}
                key={key}
              >
                <span className="flex items-center gap-3">
                  <span
                    className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-lg ${tone.icon}`}
                  >
                    <Icon aria-hidden="true" className="h-5 w-5" />
                  </span>
                  <span className="font-display text-lg font-semibold">
                    {t(`features.${key}.title`)}
                  </span>
                  <ArrowRight
                    aria-hidden="true"
                    className="ml-auto h-4 w-4 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-primary"
                  />
                </span>
                <span className="mt-3 block text-sm leading-6 text-muted-foreground">
                  {t(`features.${key}.summary`)}
                </span>
              </a>
            ))}
          </nav>
        </div>
      </section>

      {featureSections.map((feature, index) => (
        <FeatureSection
          feature={feature}
          index={index}
          key={feature.key}
          t={t}
        />
      ))}
    </main>
  );
}

function FeatureRoutePreview({ t }: { t: FeaturesTranslator }) {
  return (
    <figure
      aria-label={t("hero.preview.label")}
      className="mx-auto w-full max-w-2xl"
    >
      <figcaption className="sr-only">{t("hero.preview.caption")}</figcaption>
      <div className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950 text-white shadow-apple-card">
        <div className="flex items-center justify-between border-b border-white/10 px-4 py-3">
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-full bg-red-400" />
            <span className="h-3 w-3 rounded-full bg-amber-300" />
            <span className="h-3 w-3 rounded-full bg-emerald-400" />
          </div>
          <p className="text-xs font-medium text-zinc-400">
            {t("hero.preview.windowTitle")}
          </p>
        </div>

        <div className="grid gap-4 p-4 sm:grid-cols-[0.95fr_1.05fr]">
          <div className="rounded-lg border border-white/10 bg-white/[0.06] p-4">
            <div className="flex items-start justify-between gap-4">
              <div>
                <p className="text-xs font-medium text-zinc-400">
                  {t("hero.preview.endpointLabel")}
                </p>
                <p className="mt-2 font-mono text-sm font-semibold text-white">
                  http://127.0.0.1:8787
                </p>
              </div>
              <span className="rounded-md bg-emerald-400/15 px-2.5 py-1 text-xs font-semibold text-emerald-200">
                {t("hero.preview.status")}
              </span>
            </div>

            <ol
              aria-label={t("hero.preview.flowLabel")}
              className="mt-6 grid gap-3"
            >
              {heroFlowKeys.map((key, index) => (
                <li className="flex items-center gap-3" key={key}>
                  <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-white/10 text-xs font-semibold">
                    {index + 1}
                  </span>
                  <span className="min-w-0 flex-1 rounded-md border border-white/10 bg-black/20 px-3 py-2 text-sm text-zinc-200">
                    {t(`hero.preview.flow.${key}`)}
                  </span>
                </li>
              ))}
            </ol>
          </div>

          <div className="grid gap-3">
            <PreviewMetric
              Icon={ShieldCheck}
              label={t("hero.preview.metrics.providers.label")}
              value={t("hero.preview.metrics.providers.value")}
            />
            <PreviewMetric
              Icon={Gauge}
              label={t("hero.preview.metrics.latency.label")}
              value={t("hero.preview.metrics.latency.value")}
            />
            <PreviewMetric
              Icon={BadgeCheck}
              label={t("hero.preview.metrics.switch.label")}
              value={t("hero.preview.metrics.switch.value")}
            />
          </div>
        </div>
      </div>
    </figure>
  );
}

function PreviewMetric({
  Icon,
  label,
  value,
}: {
  Icon: LucideIcon;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-lg border border-white/10 bg-white/[0.05] p-4">
      <div className="flex items-center gap-2 text-xs font-medium text-zinc-400">
        <Icon aria-hidden="true" className="h-4 w-4 text-emerald-300" />
        {label}
      </div>
      <p className="mt-2 text-sm font-semibold text-white">{value}</p>
    </div>
  );
}

function FeatureSection({
  feature,
  index,
  t,
}: {
  feature: (typeof featureSections)[number];
  index: number;
  t: FeaturesTranslator;
}) {
  const { key, Icon, tone } = feature;
  const surfaceClassName = index % 2 === 0 ? "bg-background" : "bg-muted/40";

  return (
    <section
      aria-labelledby={`${key}-title`}
      className={`${surfaceClassName} scroll-mt-24 border-b border-border`}
      id={key}
    >
      <div className="mx-auto grid max-w-6xl gap-10 px-6 py-16 lg:grid-cols-[0.9fr_1.35fr] lg:py-20">
        <div className="lg:sticky lg:top-24 lg:self-start">
          <div
            className={`flex h-12 w-12 items-center justify-center rounded-lg ${tone.icon}`}
          >
            <Icon aria-hidden="true" className="h-6 w-6" />
          </div>
          <p className="mt-6 text-sm font-semibold text-primary">
            {t(`features.${key}.eyebrow`)}
          </p>
          <h2
            className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl"
            id={`${key}-title`}
          >
            {t(`features.${key}.title`)}
          </h2>
          <p className="mt-4 text-base leading-7 text-muted-foreground">
            {t(`features.${key}.description`)}
          </p>

          <dl className="mt-8 grid gap-3">
            {detailKeys.map((detailKey) => (
              <div
                className="rounded-lg border border-border bg-card p-4 text-card-foreground"
                key={detailKey}
              >
                <dt className="text-xs font-medium text-muted-foreground">
                  {t(`features.${key}.details.${detailKey}.label`)}
                </dt>
                <dd className="mt-2 text-sm font-semibold leading-6">
                  {t(`features.${key}.details.${detailKey}.value`)}
                </dd>
              </div>
            ))}
          </dl>
        </div>

        <div>
          <p className="sr-only">{t("screenshotsLabel")}</p>
          <div className="grid gap-4">
            {screenshotKeys.map((screenshotKey) => (
              <FeatureScreenshot
                featureKey={key}
                Icon={Icon}
                key={screenshotKey}
                screenshotKey={screenshotKey}
                t={t}
                tone={tone}
              />
            ))}
          </div>
        </div>
      </div>
    </section>
  );
}

function FeatureScreenshot({
  featureKey,
  Icon,
  screenshotKey,
  t,
  tone,
}: {
  featureKey: FeatureKey;
  Icon: LucideIcon;
  screenshotKey: ScreenshotKey;
  t: FeaturesTranslator;
  tone: (typeof featureSections)[number]["tone"];
}) {
  return (
    <Card className="overflow-hidden border-border/80 bg-card shadow-none">
      <div className="flex items-center justify-between border-b border-border bg-muted/40 px-4 py-3">
        <div className="flex items-center gap-2" aria-hidden="true">
          <span className="h-3 w-3 rounded-full bg-red-400" />
          <span className="h-3 w-3 rounded-full bg-amber-300" />
          <span className="h-3 w-3 rounded-full bg-emerald-400" />
        </div>
        <span
          className={`rounded-md px-2.5 py-1 text-xs font-semibold ${tone.soft}`}
        >
          {t(`features.${featureKey}.shots.${screenshotKey}.status`)}
        </span>
      </div>

      <CardContent className="p-5">
        <div className="flex items-start gap-4">
          <span
            className={`flex h-11 w-11 shrink-0 items-center justify-center rounded-lg ${tone.icon}`}
          >
            <Icon aria-hidden="true" className="h-5 w-5" />
          </span>
          <div>
            <h3 className="font-display text-xl font-semibold leading-7">
              {t(`features.${featureKey}.shots.${screenshotKey}.title`)}
            </h3>
            <p className="mt-2 text-sm leading-6 text-muted-foreground">
              {t(`features.${featureKey}.shots.${screenshotKey}.caption`)}
            </p>
          </div>
        </div>

        <dl className="mt-5 grid gap-3 sm:grid-cols-3">
          {detailKeys.map((rowKey) => (
            <div
              className="rounded-lg border border-border bg-background p-3"
              key={rowKey}
            >
              <dt className="text-xs font-medium text-muted-foreground">
                {t(
                  `features.${featureKey}.shots.${screenshotKey}.rows.${rowKey}.label`,
                )}
              </dt>
              <dd className="mt-2 text-sm font-semibold leading-5">
                {t(
                  `features.${featureKey}.shots.${screenshotKey}.rows.${rowKey}.value`,
                )}
              </dd>
            </div>
          ))}
        </dl>

        <div className="mt-5 rounded-lg border border-border bg-muted/40 p-4">
          <div className="h-1.5 overflow-hidden rounded-full bg-background">
            <div className={`h-full w-2/3 rounded-full ${tone.accent}`} />
          </div>
          <p className="mt-3 text-sm leading-6 text-muted-foreground">
            {t(`features.${featureKey}.shots.${screenshotKey}.footer`)}
          </p>
        </div>
      </CardContent>
    </Card>
  );
}
