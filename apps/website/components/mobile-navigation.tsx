"use client";

import { Menu, X } from "lucide-react";
import { useState } from "react";

type MobileNavigationItem = {
  href: string;
  label: string;
};

type MobileNavigationProps = {
  closeLabel: string;
  items: MobileNavigationItem[];
  label: string;
  menuId: string;
  openLabel: string;
};

export function MobileNavigation({
  closeLabel,
  items,
  label,
  menuId,
  openLabel,
}: MobileNavigationProps) {
  const [open, setOpen] = useState(false);
  const Icon = open ? X : Menu;

  return (
    <div className="sm:hidden">
      <button
        aria-controls={menuId}
        aria-expanded={open}
        aria-label={open ? closeLabel : openLabel}
        className="flex h-9 w-9 items-center justify-center rounded-md border border-border bg-background text-foreground transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
        onClick={() => setOpen((current) => !current)}
        type="button"
      >
        <Icon aria-hidden="true" className="h-4 w-4" />
      </button>

      <nav
        aria-label={label}
        className={
          open
            ? "absolute left-0 right-0 top-full border-b border-border bg-background px-6 py-4 shadow-lg"
            : "hidden"
        }
        id={menuId}
      >
        <ul className="grid gap-2">
          {items.map((item) => (
            <li key={item.href}>
              <a
                className="block rounded-md px-3 py-2 text-sm font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
                href={item.href}
                onClick={() => setOpen(false)}
              >
                {item.label}
              </a>
            </li>
          ))}
        </ul>
      </nav>
    </div>
  );
}
