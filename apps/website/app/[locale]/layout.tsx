import { NextIntlClientProvider } from "next-intl";
import {
  getMessages,
  getTranslations,
  setRequestLocale,
} from "next-intl/server";
import type { Metadata, Viewport } from "next";
import { notFound } from "next/navigation";
import type { ReactNode } from "react";

import "../globals.css";
import { SiteFooter } from "../../components/site-footer";
import { SiteHeader } from "../../components/site-header";
import { ThemeProvider } from "../../components/theme-provider";
import { defaultLocale, isLocale, locales } from "../../i18n/routing";
import { absoluteUrl, siteName, siteUrl } from "../../site";

type LocaleLayoutProps = {
  children: ReactNode;
  params: {
    locale: string;
  };
};

export function generateStaticParams() {
  return locales.map((locale) => ({ locale }));
}

export async function generateMetadata({
  params,
}: LocaleLayoutProps): Promise<Metadata> {
  const locale = isLocale(params.locale) ? params.locale : defaultLocale;
  const t = await getTranslations({ locale, namespace: "Metadata" });
  const description = t("description");
  const canonical = `/${locale}`;
  const openGraphImage = absoluteUrl("/opengraph-image.png");
  const languages = Object.fromEntries(
    locales.map((item) => [item, `/${item}`]),
  );

  return {
    metadataBase: new URL(siteUrl),
    title: {
      default: siteName,
      template: `%s | ${siteName}`,
    },
    description,
    applicationName: siteName,
    alternates: {
      canonical,
      languages: {
        ...languages,
        "x-default": `/${defaultLocale}`,
      },
    },
    openGraph: {
      title: siteName,
      description,
      siteName,
      url: absoluteUrl(canonical),
      type: "website",
      locale: locale === "zh" ? "zh_CN" : "en_US",
      alternateLocale: locale === "zh" ? ["en_US"] : ["zh_CN"],
      images: [
        {
          url: openGraphImage,
          width: 801,
          height: 801,
          alt: siteName,
        },
      ],
    },
    twitter: {
      card: "summary",
      title: siteName,
      description,
      images: [openGraphImage],
    },
  };
}

export const viewport: Viewport = {
  width: "device-width",
  initialScale: 1,
  themeColor: "#000000",
};

export default async function LocaleLayout({
  children,
  params,
}: LocaleLayoutProps) {
  const { locale } = params;

  if (!isLocale(locale)) {
    notFound();
  }

  setRequestLocale(locale);
  const messages = await getMessages();

  return (
    <html lang={locale} suppressHydrationWarning>
      <body className="min-h-screen bg-background text-foreground">
        <NextIntlClientProvider messages={messages}>
          <ThemeProvider>
            <SiteHeader locale={locale} />
            {children}
            <SiteFooter locale={locale} />
          </ThemeProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
