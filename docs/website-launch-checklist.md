# CCUse Website Vercel Setup

- **Scope**: Phase 1.0.W.3 repository-side Vercel configuration
- **App**: `apps/website`
- **Deployment owner**: repository owner via Vercel
- **Current status**: Vercel config is prepared. Custom domain, DNS, production deployment, Lighthouse, and cross-browser launch checks are intentionally deferred.

## Vercel Project Settings

Create or import the Vercel project from the Git repository and set these values:

| Item             | Value                                            |
| ---------------- | ------------------------------------------------ |
| Root directory   | `apps/website`                                   |
| Framework        | Next.js                                          |
| Install command  | `cd ../.. && pnpm install --frozen-lockfile`     |
| Build command    | `cd ../.. && pnpm --filter @ccuse/website build` |
| Output directory | `.next`                                          |

The same values are checked into `apps/website/vercel.json`, so Vercel can read them after the project root is set to `apps/website`.

## Environment Variables

| Name                           | Required                                                | Purpose                                                                                                                  |
| ------------------------------ | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `NEXT_PUBLIC_SITE_URL`         | Optional for Vercel preview, recommended for production | Canonical site URL. Leave unset before custom domain binding; the app falls back to Vercel-provided URLs when available. |
| `NEXT_PUBLIC_PLAUSIBLE_DOMAIN` | Optional                                                | Enables privacy-friendly analytics when set.                                                                             |
| `NEXT_PUBLIC_PLAUSIBLE_SRC`    | Optional                                                | Overrides the Plausible script URL.                                                                                      |

No GitHub Actions deploy workflow is configured for the website. Vercel should build and deploy the site from its own Git integration.

## Repository Evidence

| Area                   | Evidence                                                                         |
| ---------------------- | -------------------------------------------------------------------------------- |
| Vercel config          | `apps/website/vercel.json`                                                       |
| Canonical URL fallback | `apps/website/site.ts` reads `NEXT_PUBLIC_SITE_URL`, then Vercel URL env values  |
| Analytics              | `apps/website/components/site-analytics.tsx` uses `next/script` and is env-gated |
| Mobile navigation      | `apps/website/components/mobile-navigation.tsx`                                  |
| Legal pages            | `/[locale]/legal/privacy`, `/[locale]/legal/terms`                               |
| Download pages         | `/[locale]/download`, `/[locale]/download/preview`                               |

## Deferred Items

These are not part of the current repo-side task:

- Custom domain / DNS binding
- Manual production deploy operation
- GitHub Actions based Vercel deployment
- Lighthouse report collection
- Chrome / Safari / Firefox / Edge launch signoff
- Social launch announcement and first-day analytics review
