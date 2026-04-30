"use client";

import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import { CheckCircle2, Download, Laptop, MonitorDown } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  detectPlatformFromUserAgent,
  findRecommendedAsset,
  platformRecommendationIds,
  type DownloadAssetCandidate,
  type PlatformRecommendationId,
} from "../lib/download-platform";

type PlatformOptionLabel = {
  description: string;
  title: string;
};

type DownloadPlatformRecommendationLabels = {
  detectedLabel: string;
  description: string;
  downloadRecommended: string;
  noAsset: string;
  options: Record<
    Exclude<PlatformRecommendationId, "unknown">,
    PlatformOptionLabel
  >;
  title: string;
  unknownDescription: string;
  unknownTitle: string;
};

type DownloadPlatformRecommendationProps = {
  assets: DownloadAssetCandidate[];
  labels: DownloadPlatformRecommendationLabels;
};

type NavigatorWithUserAgentData = Navigator & {
  userAgentData?: {
    getHighEntropyValues?: (hints: string[]) => Promise<{
      architecture?: string;
      platform?: string;
    }>;
    platform?: string;
  };
};

export function DownloadPlatformRecommendation({
  assets,
  labels,
}: DownloadPlatformRecommendationProps) {
  const [platformId, setPlatformId] =
    useState<PlatformRecommendationId>("unknown");
  const recommendedAsset = useMemo(
    () => findRecommendedAsset(assets, platformId),
    [assets, platformId],
  );

  useEffect(() => {
    let cancelled = false;
    const navigatorWithHints = window.navigator as NavigatorWithUserAgentData;

    async function detectPlatform() {
      const userAgent = navigatorWithHints.userAgent;
      const platform = navigatorWithHints.platform;
      const userAgentDataPlatform = navigatorWithHints.userAgentData?.platform;
      let architecture = "";

      try {
        const values =
          await navigatorWithHints.userAgentData?.getHighEntropyValues?.([
            "architecture",
            "platform",
          ]);
        architecture = values?.architecture ?? "";
      } catch {
        architecture = "";
      }

      if (!cancelled) {
        setPlatformId(
          detectPlatformFromUserAgent({
            architecture,
            platform,
            userAgent,
            userAgentDataPlatform,
          }),
        );
      }
    }

    void detectPlatform();

    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <Card
      aria-labelledby="platform-recommendation-title"
      className="border-border/80 bg-card shadow-none"
    >
      <CardContent className="p-6">
        <div className="flex flex-col gap-5 lg:flex-row lg:items-start lg:justify-between">
          <div className="max-w-2xl">
            <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-primary/10 text-primary">
              <MonitorDown aria-hidden="true" className="h-6 w-6" />
            </div>
            <h2
              className="mt-5 font-display text-2xl font-semibold"
              id="platform-recommendation-title"
            >
              {labels.title}
            </h2>
            <p className="mt-3 text-sm leading-6 text-muted-foreground">
              {labels.description}
            </p>
          </div>

          <div className="rounded-lg border border-border bg-muted/40 px-4 py-3">
            <p className="text-xs font-medium text-muted-foreground">
              {labels.detectedLabel}
            </p>
            <p className="mt-1 text-sm font-semibold">
              {platformId === "unknown"
                ? labels.unknownTitle
                : labels.options[platformId].title}
            </p>
          </div>
        </div>

        <div className="mt-6 grid gap-3 lg:grid-cols-3">
          {platformRecommendationIds.map((id) => {
            const active = platformId === id;

            return (
              <div
                className={
                  active
                    ? "rounded-lg border border-primary bg-primary/10 p-4 text-primary"
                    : "rounded-lg border border-border bg-background p-4"
                }
                key={id}
              >
                <div className="flex items-center gap-3">
                  <Laptop aria-hidden="true" className="h-5 w-5" />
                  <h3 className="text-sm font-semibold">
                    {labels.options[id].title}
                  </h3>
                  {active ? (
                    <CheckCircle2
                      aria-hidden="true"
                      className="ml-auto h-5 w-5"
                    />
                  ) : null}
                </div>
                <p
                  className={
                    active
                      ? "mt-3 text-sm leading-6 text-primary/90"
                      : "mt-3 text-sm leading-6 text-muted-foreground"
                  }
                >
                  {labels.options[id].description}
                </p>
              </div>
            );
          })}
        </div>

        <div className="mt-6 rounded-lg border border-border bg-muted/40 p-4">
          {platformId === "unknown" ? (
            <p className="text-sm leading-6 text-muted-foreground">
              {labels.unknownDescription}
            </p>
          ) : recommendedAsset ? (
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
              <div>
                <p className="text-sm font-semibold">{recommendedAsset.name}</p>
                <p className="mt-1 text-xs text-muted-foreground">
                  {recommendedAsset.sizeLabel}
                </p>
              </div>
              <Button asChild>
                <a href={recommendedAsset.downloadUrl}>
                  <Download aria-hidden="true" />
                  {labels.downloadRecommended}
                </a>
              </Button>
            </div>
          ) : (
            <p className="text-sm leading-6 text-muted-foreground">
              {labels.noAsset}
            </p>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
