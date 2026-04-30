"use client";

import { Search } from "lucide-react";
import { useEffect, useId, useState } from "react";

import type { Locale } from "../i18n/routing";

type DocsSearchLabels = {
  empty: string;
  error: string;
  label: string;
  loading: string;
  placeholder: string;
};

type DocsSearchProps = {
  labels: DocsSearchLabels;
  locale: Locale;
};

type PagefindModule = {
  search: (query: string) => Promise<PagefindSearch>;
};

type PagefindSearch = {
  results: PagefindSearchResult[];
};

type PagefindSearchResult = {
  data: () => Promise<PagefindResultData>;
  id: string;
};

type PagefindResultData = {
  excerpt: string;
  meta: {
    title?: string;
  };
  url: string;
};

type SearchResult = {
  excerpt: string;
  title: string;
  url: string;
};

type SearchStatus = "idle" | "loading" | "ready" | "error";

let pagefindPromise: Promise<PagefindModule> | null = null;

function loadPagefind() {
  const pagefindPath = "/_pagefind/pagefind.js";

  pagefindPromise ??= import(
    /* webpackIgnore: true */ pagefindPath
  ) as Promise<PagefindModule>;

  return pagefindPromise;
}

function normalizePagefindUrl(url: string) {
  const parsed = new URL(url, window.location.origin);
  const normalizedPathname = parsed.pathname.endsWith(".html")
    ? parsed.pathname.slice(0, -5)
    : parsed.pathname;

  return `${normalizedPathname}${parsed.hash}`;
}

function cleanExcerpt(excerpt: string) {
  return excerpt
    .replace(/<\/?mark>/g, "")
    .replace(/<[^>]*>/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

export function DocsSearch({ labels, locale }: DocsSearchProps) {
  const inputId = useId();
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [status, setStatus] = useState<SearchStatus>("idle");

  useEffect(() => {
    const trimmedQuery = query.trim();

    if (trimmedQuery.length < 2) {
      setResults([]);
      setStatus("idle");
      return;
    }

    let canceled = false;

    setStatus("loading");

    const timeoutId = window.setTimeout(async () => {
      try {
        const pagefind = await loadPagefind();
        const search = await pagefind.search(trimmedQuery);
        const resultData = await Promise.all(
          search.results.slice(0, 10).map((result) => result.data()),
        );

        if (canceled) {
          return;
        }

        setResults(
          resultData
            .map((result) => ({
              excerpt: cleanExcerpt(result.excerpt),
              title: result.meta.title ?? result.url,
              url: normalizePagefindUrl(result.url),
            }))
            .filter((result) => result.url.startsWith(`/${locale}/`))
            .slice(0, 5),
        );
        setStatus("ready");
      } catch {
        if (!canceled) {
          setResults([]);
          setStatus("error");
        }
      }
    }, 160);

    return () => {
      canceled = true;
      window.clearTimeout(timeoutId);
    };
  }, [locale, query]);

  const showEmpty = status === "ready" && results.length === 0;

  return (
    <div className="mt-4" data-pagefind-ignore>
      <label className="sr-only" htmlFor={inputId}>
        {labels.label}
      </label>
      <div className="relative">
        <Search
          aria-hidden="true"
          className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <input
          aria-label={labels.label}
          className="h-10 w-full rounded-lg border border-border bg-background pl-9 pr-3 text-sm outline-none transition-colors placeholder:text-muted-foreground focus:border-primary focus:ring-2 focus:ring-ring"
          id={inputId}
          onChange={(event) => setQuery(event.target.value)}
          placeholder={labels.placeholder}
          type="search"
          value={query}
        />
      </div>
      {status === "loading" ? (
        <p className="mt-3 text-xs text-muted-foreground">{labels.loading}</p>
      ) : null}
      {status === "error" ? (
        <p className="mt-3 text-xs text-destructive">{labels.error}</p>
      ) : null}
      {showEmpty ? (
        <p className="mt-3 text-xs text-muted-foreground">{labels.empty}</p>
      ) : null}
      {results.length > 0 ? (
        <ul className="mt-3 grid gap-2">
          {results.map((result) => (
            <li key={result.url}>
              <a
                className="block rounded-md px-3 py-2 text-sm transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                href={result.url}
              >
                <span className="font-medium text-foreground">
                  {result.title}
                </span>
                {result.excerpt ? (
                  <span className="mt-1 block text-xs leading-5 text-muted-foreground">
                    {result.excerpt}
                  </span>
                ) : null}
              </a>
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
