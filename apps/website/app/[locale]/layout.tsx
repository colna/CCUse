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
import { defaultLocale, isLocale, locales } from "../../i18n/routing";

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

  return {
    title: {
      default: "CCUse",
      template: "%s | CCUse",
    },
    description: t("description"),
    applicationName: "CCUse",
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
    <html lang={locale}>
      <body className="min-h-screen">
        <NextIntlClientProvider messages={messages}>
          {children}
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
