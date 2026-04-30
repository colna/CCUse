import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import {
  Activity,
  ArrowRight,
  BarChart3,
  BrainCircuit,
  Download,
  Gauge,
  Github,
  HeartPulse,
  Laptop,
  Network,
  ServerCog,
  ShieldCheck,
  Shuffle,
} from "lucide-react";
import Image from "next/image";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import { isLocale } from "../../i18n/routing";
import { architectureMermaid } from "../../lib/architecture";

const heroMetricKeys = [
  { key: "endpoint", Icon: ServerCog },
  { key: "providers", Icon: ShieldCheck },
  { key: "failover", Icon: Gauge },
] as const;
const featureItems = [
  {
    key: "failover",
    Icon: Shuffle,
    iconClassName: "bg-emerald-500/10 text-emerald-600 dark:text-emerald-300",
  },
  {
    key: "multiProvider",
    Icon: Network,
    iconClassName: "bg-sky-500/10 text-sky-600 dark:text-sky-300",
  },
  {
    key: "healthCheck",
    Icon: HeartPulse,
    iconClassName: "bg-rose-500/10 text-rose-600 dark:text-rose-300",
  },
  {
    key: "smartStrategy",
    Icon: BrainCircuit,
    iconClassName: "bg-violet-500/10 text-violet-600 dark:text-violet-300",
  },
  {
    key: "monitoring",
    Icon: BarChart3,
    iconClassName: "bg-amber-500/10 text-amber-600 dark:text-amber-300",
  },
  {
    key: "crossPlatform",
    Icon: Laptop,
    iconClassName: "bg-zinc-500/10 text-zinc-700 dark:text-zinc-200",
  },
] as const;
const providerRows = [
  {
    key: "anthropic",
    dotClassName: "bg-emerald-400",
    barClassName: "w-[92%] bg-emerald-400",
  },
  {
    key: "openai",
    dotClassName: "bg-amber-300",
    barClassName: "w-[54%] bg-amber-300",
  },
  {
    key: "gemini",
    dotClassName: "bg-sky-400",
    barClassName: "w-[72%] bg-sky-400",
  },
] as const;
const trafficSteps = ["client", "proxy", "provider"] as const;
const architectureColumns = [
  { key: "client", Icon: Laptop, nodeKeys: ["tools", "key", "baseUrl"] },
  {
    key: "proxy",
    Icon: ServerCog,
    nodeKeys: ["auth", "parse", "strategy", "forward"],
  },
  {
    key: "providers",
    Icon: Network,
    nodeKeys: ["claude", "openai", "gemini"],
  },
] as const;
const architectureFlowKeys = ["request", "selection", "recovery"] as const;

type HomePageProps = {
  params: {
    locale: string;
  };
};

export default async function HomePage({ params }: HomePageProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const t = await getTranslations({ locale, namespace: "HomePage" });

  return (
    <main className="bg-background text-foreground">
      <section
        aria-labelledby="hero-title"
        className="overflow-hidden border-b border-border bg-background"
      >
        <div className="mx-auto grid max-w-6xl items-center gap-12 px-6 py-14 sm:py-16 lg:grid-cols-[0.95fr_1.05fr] lg:py-20">
          <div>
            <div className="inline-flex items-center gap-3 text-sm font-semibold text-primary">
              <Image
                alt=""
                className="h-9 w-9 rounded-lg"
                height={36}
                priority
                src="/icon.png"
                width={36}
              />
              <span>{t("eyebrow")}</span>
            </div>

            <h1
              id="hero-title"
              className="mt-6 font-display text-5xl font-semibold leading-apple-headline sm:text-6xl"
            >
              {t("title")}
            </h1>
            <p className="mt-5 max-w-2xl font-display text-2xl font-semibold leading-8 text-foreground sm:text-3xl sm:leading-10">
              {t("slogan")}
            </p>
            <p className="mt-5 max-w-2xl text-base leading-7 text-muted-foreground sm:text-lg sm:leading-8">
              {t("description")}
            </p>

            <nav
              aria-label={t("actions.label")}
              className="mt-8 flex flex-col gap-3 sm:flex-row"
            >
              <Button asChild size="lg">
                <a href={`/${locale}/download`}>
                  <Download aria-hidden="true" />
                  {t("actions.download")}
                </a>
              </Button>
              <Button asChild size="lg" variant="secondary">
                <a href="https://github.com/colna/CCUse">
                  <Github aria-hidden="true" />
                  {t("actions.github")}
                </a>
              </Button>
            </nav>

            <dl className="mt-10 grid gap-3 sm:grid-cols-3">
              {heroMetricKeys.map(({ key, Icon }) => (
                <div
                  className="rounded-lg border border-border bg-card p-4 text-card-foreground"
                  key={key}
                >
                  <dt className="flex items-center gap-2 text-xs font-medium text-muted-foreground">
                    <Icon aria-hidden="true" className="h-4 w-4 text-primary" />
                    {t(`heroMetrics.${key}.label`)}
                  </dt>
                  <dd className="mt-2 text-sm font-semibold">
                    {t(`heroMetrics.${key}.value`)}
                  </dd>
                </div>
              ))}
            </dl>
          </div>

          <HeroProductPreview t={t} />
        </div>
      </section>
      <section
        aria-labelledby="features-title"
        id="features"
        className="bg-muted/40"
      >
        <div className="mx-auto max-w-6xl px-6 py-16">
          <div className="max-w-2xl">
            <p className="text-sm font-semibold text-primary">
              {t("featuresEyebrow")}
            </p>
            <h2
              id="features-title"
              className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl"
            >
              {t("featuresTitle")}
            </h2>
            <p className="mt-4 text-base leading-7 text-muted-foreground">
              {t("featuresDescription")}
            </p>
          </div>

          <ul className="mt-10 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {featureItems.map(({ key, Icon, iconClassName }) => (
              <li key={key}>
                <a
                  className="group block h-full rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                  href={`/${locale}/features#${key}`}
                >
                  <Card className="h-full border-border/70 bg-card shadow-none transition-colors group-hover:border-primary/50">
                    <CardContent className="flex h-full flex-col p-6">
                      <div
                        className={`flex h-10 w-10 items-center justify-center rounded-lg ${iconClassName}`}
                      >
                        <Icon aria-hidden="true" className="h-5 w-5" />
                      </div>
                      <h3 className="mt-5 font-display text-xl font-semibold leading-7">
                        {t(`features.${key}.title`)}
                      </h3>
                      <p className="mt-3 text-sm leading-6 text-muted-foreground">
                        {t(`features.${key}.description`)}
                      </p>
                      <span className="mt-auto flex items-center gap-2 pt-5 text-sm font-semibold text-primary">
                        {t("featuresCta")}
                        <ArrowRight
                          aria-hidden="true"
                          className="h-4 w-4 transition-transform group-hover:translate-x-0.5"
                        />
                      </span>
                    </CardContent>
                  </Card>
                </a>
              </li>
            ))}
          </ul>
        </div>
      </section>
      <ArchitectureSection t={t} />
    </main>
  );
}

type HomeTranslator = Awaited<ReturnType<typeof getTranslations>>;

function HeroProductPreview({ t }: { t: HomeTranslator }) {
  return (
    <figure
      aria-label={t("heroPreview.label")}
      className="relative mx-auto w-full max-w-xl"
    >
      <figcaption className="sr-only">{t("heroPreview.caption")}</figcaption>
      <div className="overflow-hidden rounded-lg border border-zinc-800 bg-zinc-950 text-white shadow-apple-card">
        <div className="flex items-center justify-between border-b border-white/10 px-4 py-3">
          <div className="flex items-center gap-2">
            <span className="h-3 w-3 rounded-full bg-red-400" />
            <span className="h-3 w-3 rounded-full bg-amber-300" />
            <span className="h-3 w-3 rounded-full bg-emerald-400" />
          </div>
          <p className="text-xs font-medium text-zinc-400">
            {t("heroPreview.windowTitle")}
          </p>
        </div>

        <div className="grid gap-4 p-4 sm:grid-cols-[1fr_0.9fr]">
          <div className="rounded-lg border border-white/10 bg-white/[0.06] p-4">
            <div className="flex items-center justify-between gap-4">
              <div>
                <p className="text-xs font-medium text-zinc-400">
                  {t("heroPreview.endpointLabel")}
                </p>
                <p className="mt-2 font-mono text-sm font-semibold text-white">
                  http://127.0.0.1:8787
                </p>
              </div>
              <span className="rounded-md bg-emerald-400/15 px-2.5 py-1 text-xs font-semibold text-emerald-200">
                {t("heroPreview.live")}
              </span>
            </div>

            <ol
              aria-label={t("heroPreview.trafficLabel")}
              className="mt-6 grid gap-3"
            >
              {trafficSteps.map((step, index) => (
                <li className="flex items-center gap-3" key={step}>
                  <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-white/10 text-xs font-semibold text-white">
                    {index + 1}
                  </span>
                  <span className="min-w-0 flex-1 rounded-md border border-white/10 bg-black/20 px-3 py-2 text-sm text-zinc-200">
                    {t(`heroPreview.traffic.${step}`)}
                  </span>
                </li>
              ))}
            </ol>
          </div>

          <div className="rounded-lg border border-white/10 bg-black/30 p-4">
            <div className="flex items-center justify-between">
              <p className="text-xs font-medium uppercase text-zinc-400">
                {t("heroPreview.providersLabel")}
              </p>
              <Activity
                aria-hidden="true"
                className="h-4 w-4 text-emerald-300 motion-safe:animate-pulse"
              />
            </div>
            <ul className="mt-4 grid gap-3">
              {providerRows.map((provider) => (
                <li
                  className="rounded-md border border-white/10 bg-white/[0.04] p-3"
                  key={provider.key}
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2">
                      <span
                        className={`h-2.5 w-2.5 rounded-full ${provider.dotClassName}`}
                      />
                      <span className="truncate text-sm font-medium">
                        {t(`heroPreview.providers.${provider.key}.name`)}
                      </span>
                    </div>
                    <span className="text-xs text-zinc-400">
                      {t(`heroPreview.providers.${provider.key}.status`)}
                    </span>
                  </div>
                  <div className="mt-3 h-1.5 overflow-hidden rounded-full bg-white/10">
                    <div
                      className={`h-full rounded-full ${provider.barClassName}`}
                    />
                  </div>
                </li>
              ))}
            </ul>
          </div>
        </div>

        <div className="border-t border-white/10 bg-white/[0.04] px-4 py-3">
          <p className="text-sm text-zinc-300">
            <span className="font-semibold text-emerald-200">
              {t("heroPreview.failoverLead")}
            </span>{" "}
            {t("heroPreview.failoverText")}
          </p>
        </div>
      </div>
    </figure>
  );
}

function ArchitectureSection({ t }: { t: HomeTranslator }) {
  return (
    <section
      aria-labelledby="architecture-title"
      className="bg-background"
      id="architecture"
    >
      <div className="mx-auto max-w-6xl px-6 py-16">
        <div className="max-w-2xl">
          <p className="text-sm font-semibold text-primary">
            {t("architecture.eyebrow")}
          </p>
          <h2
            className="mt-3 font-display text-3xl font-semibold leading-apple-tile sm:text-4xl"
            id="architecture-title"
          >
            {t("architecture.title")}
          </h2>
          <p className="mt-4 text-base leading-7 text-muted-foreground">
            {t("architecture.description")}
          </p>
        </div>

        <div
          aria-label={t("architecture.diagramLabel")}
          className="mt-10 hidden rounded-lg border border-border bg-card p-5 text-card-foreground lg:block"
          role="img"
        >
          <div className="grid grid-cols-[1fr_auto_1.2fr_auto_1fr] items-stretch gap-4">
            {architectureColumns.map(({ key, Icon, nodeKeys }, index) => (
              <FragmentWithArrow
                arrowLabel={
                  index < architectureFlowKeys.length - 1
                    ? t(`architecture.flows.${architectureFlowKeys[index]}`)
                    : undefined
                }
                key={key}
                showArrow={index < architectureColumns.length - 1}
              >
                <div className="flex h-full flex-col rounded-lg border border-border/80 bg-background p-5">
                  <div className="flex items-center gap-3">
                    <span className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10 text-primary">
                      <Icon aria-hidden="true" className="h-5 w-5" />
                    </span>
                    <h3 className="font-display text-xl font-semibold">
                      {t(`architecture.columns.${key}.title`)}
                    </h3>
                  </div>
                  <ul className="mt-5 grid gap-3">
                    {nodeKeys.map((nodeKey) => (
                      <li
                        className="rounded-md border border-border bg-muted/50 px-3 py-2 text-sm"
                        key={nodeKey}
                      >
                        {t(`architecture.columns.${key}.nodes.${nodeKey}`)}
                      </li>
                    ))}
                  </ul>
                </div>
              </FragmentWithArrow>
            ))}
          </div>

          <div className="mt-5 grid gap-3 rounded-lg border border-border bg-muted/40 p-4 sm:grid-cols-3">
            {architectureFlowKeys.map((key) => (
              <p className="text-sm leading-6 text-muted-foreground" key={key}>
                <span className="font-semibold text-foreground">
                  {t(`architecture.flows.${key}`)}
                </span>{" "}
                {t(`architecture.flowDescriptions.${key}`)}
              </p>
            ))}
          </div>

          <pre className="sr-only">{architectureMermaid}</pre>
        </div>

        <div className="mt-10 lg:hidden">
          <Image
            alt={t("architecture.mobileAlt")}
            className="w-full rounded-lg border border-border bg-card"
            height={520}
            src="/architecture-mobile.svg"
            width={720}
          />
          <ol className="mt-5 grid gap-3">
            {architectureFlowKeys.map((key, index) => (
              <li
                className="flex gap-3 rounded-lg border border-border bg-card p-4 text-sm"
                key={key}
              >
                <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary/10 font-semibold text-primary">
                  {index + 1}
                </span>
                <span className="leading-6 text-muted-foreground">
                  <span className="font-semibold text-foreground">
                    {t(`architecture.flows.${key}`)}
                  </span>{" "}
                  {t(`architecture.flowDescriptions.${key}`)}
                </span>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </section>
  );
}

function FragmentWithArrow({
  arrowLabel,
  children,
  showArrow,
}: {
  arrowLabel?: string;
  children: ReactNode;
  showArrow: boolean;
}) {
  return (
    <>
      {children}
      {showArrow ? (
        <div className="flex min-w-24 flex-col items-center justify-center gap-2 text-center text-xs font-medium text-muted-foreground">
          <ArrowRight aria-hidden="true" className="h-6 w-6 text-primary" />
          <span>{arrowLabel}</span>
        </div>
      ) : null}
    </>
  );
}
