import Script from "next/script";

import { analyticsDomain, analyticsScriptSrc } from "../site";

export function SiteAnalytics() {
  if (!analyticsDomain) {
    return null;
  }

  return (
    <Script
      data-domain={analyticsDomain}
      defer
      src={analyticsScriptSrc}
      strategy="afterInteractive"
    />
  );
}
