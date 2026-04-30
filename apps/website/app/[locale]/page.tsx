import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";
import { getTranslations, setRequestLocale } from "next-intl/server";
import { notFound } from "next/navigation";

import { isLocale } from "../../i18n/routing";

const capabilityKeys = ["proxy", "routing", "failover"] as const;

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
    <main className="min-h-screen bg-background text-foreground">
      <section
        aria-labelledby="hero-title"
        className="mx-auto flex min-h-screen max-w-5xl flex-col justify-center px-6 py-24"
      >
        <p className="text-sm font-semibold text-primary">{t("eyebrow")}</p>
        <h1
          id="hero-title"
          className="mt-5 max-w-3xl font-display text-5xl font-semibold leading-apple-headline tracking-apple-tighter"
        >
          {t("title")}
        </h1>
        <p className="mt-6 max-w-2xl text-lg leading-8 text-muted-foreground">
          {t("description")}
        </p>
        <nav aria-label="Primary actions" className="mt-8 flex gap-3">
          <Button asChild>
            <a href={`/${locale}/download`}>{t("actions.download")}</a>
          </Button>
          <Button asChild variant="secondary">
            <a href="https://github.com/colna/CCUse">{t("actions.github")}</a>
          </Button>
        </nav>
      </section>
      <section
        aria-labelledby="capabilities-title"
        id="features"
        className="mx-auto grid max-w-5xl gap-6 px-6 pb-24 sm:grid-cols-3"
      >
        <h2 id="capabilities-title" className="sr-only">
          {t("capabilitiesTitle")}
        </h2>
        <ul className="contents">
          {capabilityKeys.map((key) => (
            <li key={key}>
              <Card className="shadow-apple-card">
                <CardContent className="p-5 text-sm font-medium">
                  {t(`capabilities.${key}`)}
                </CardContent>
              </Card>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}
