"use client";

import { useEffect, useState } from "react";

import type { DocsTocItem } from "../lib/docs";

type DocsTableOfContentsProps = {
  items: DocsTocItem[];
  label: string;
};

export function DocsTableOfContents({
  items,
  label,
}: DocsTableOfContentsProps) {
  const [activeId, setActiveId] = useState(items[0]?.id ?? "");

  useEffect(() => {
    if (items.length === 0) {
      return;
    }

    const headings = items
      .map((item) => document.getElementById(item.id))
      .filter((heading): heading is HTMLElement => Boolean(heading));

    if (headings.length === 0) {
      return;
    }

    const updateActiveHeading = () => {
      let currentHeading = headings[0];

      for (const heading of headings) {
        if (heading.getBoundingClientRect().top <= 112) {
          currentHeading = heading;
        }
      }

      setActiveId(currentHeading.id);
    };

    const observer = new IntersectionObserver(updateActiveHeading, {
      rootMargin: "-112px 0px -70% 0px",
      threshold: [0, 1],
    });

    for (const heading of headings) {
      observer.observe(heading);
    }

    updateActiveHeading();
    window.addEventListener("scroll", updateActiveHeading, { passive: true });

    return () => {
      window.removeEventListener("scroll", updateActiveHeading);
      observer.disconnect();
    };
  }, [items]);

  if (items.length === 0) {
    return null;
  }

  return (
    <nav
      aria-label={label}
      className="hidden xl:sticky xl:top-24 xl:block xl:max-h-[calc(100vh-7rem)] xl:overflow-y-auto"
    >
      <p className="text-xs font-semibold uppercase text-muted-foreground">
        {label}
      </p>
      <ol className="mt-3 grid gap-1 border-l border-border">
        {items.map((item) => {
          const isActive = item.id === activeId;

          return (
            <li className={item.depth === 3 ? "pl-4" : ""} key={item.id}>
              <a
                aria-current={isActive ? "true" : undefined}
                className={[
                  "block border-l px-3 py-1.5 text-sm leading-5 transition-colors",
                  isActive
                    ? "border-primary text-foreground"
                    : "border-transparent text-muted-foreground hover:text-foreground",
                ].join(" ")}
                href={`#${item.id}`}
              >
                {item.title}
              </a>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}
