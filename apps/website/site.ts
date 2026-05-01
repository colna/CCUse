export const siteName = "CCUse";
const vercelSiteUrl =
  process.env.VERCEL_PROJECT_PRODUCTION_URL ?? process.env.VERCEL_URL;

export const siteUrl =
  process.env.NEXT_PUBLIC_SITE_URL ??
  (vercelSiteUrl ? withHttps(vercelSiteUrl) : "https://ccuse.app");
export const analyticsDomain = process.env.NEXT_PUBLIC_PLAUSIBLE_DOMAIN ?? "";
export const analyticsScriptSrc =
  process.env.NEXT_PUBLIC_PLAUSIBLE_SRC ?? "https://plausible.io/js/script.js";

export function absoluteUrl(pathname: string) {
  return new URL(pathname, siteUrl).toString();
}

function withHttps(url: string) {
  return url.startsWith("http://") || url.startsWith("https://")
    ? url
    : `https://${url}`;
}
