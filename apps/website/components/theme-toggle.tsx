"use client";

import { useTheme } from "next-themes";
import { useEffect, useState } from "react";

type ThemeMode = "system" | "light" | "dark";

type ThemeToggleProps = {
  ariaLabel: string;
  labels: Record<ThemeMode, string>;
};

const modes: ThemeMode[] = ["system", "light", "dark"];

export function ThemeToggle({ ariaLabel, labels }: ThemeToggleProps) {
  const { setTheme, theme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  const activeTheme = mounted && isThemeMode(theme) ? theme : "system";

  return (
    <div
      aria-label={ariaLabel}
      className="flex rounded-md border border-border bg-muted/50 p-0.5 text-xs font-medium"
      role="group"
    >
      {modes.map((mode) => {
        const active = activeTheme === mode;

        return (
          <button
            aria-pressed={active}
            className={
              active
                ? "rounded bg-background px-2.5 py-1 text-foreground shadow-sm"
                : "rounded px-2.5 py-1 text-muted-foreground transition-colors hover:text-foreground"
            }
            key={mode}
            onClick={() => setTheme(mode)}
            type="button"
          >
            {labels[mode]}
          </button>
        );
      })}
    </div>
  );
}

function isThemeMode(value: string | undefined): value is ThemeMode {
  return value === "system" || value === "light" || value === "dark";
}
