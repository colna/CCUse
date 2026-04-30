export const siteName = "CCUse";
export const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? "https://ccuse.app";

export function absoluteUrl(pathname: string) {
  return new URL(pathname, siteUrl).toString();
}
