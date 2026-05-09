import { useTranslation } from "react-i18next";

interface TopbarProps {
  title: string;
  description?: string;
}

export function Topbar({ title, description }: TopbarProps) {
  const { t } = useTranslation("common");

  return (
    <header className="bg-[var(--ant-color-bg-container,#fff)]/60 flex h-16 items-center justify-between border-b border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] px-8 backdrop-blur">
      <div>
        <h1 className="text-lg font-semibold leading-apple-headline tracking-apple-tight">
          {title}
        </h1>
        {description ? (
          <p className="mt-0.5 text-xs leading-snug text-muted-foreground">
            {description}
          </p>
        ) : null}
      </div>
      <span className="inline-flex items-center gap-2 rounded-full border border-[var(--ant-color-border-secondary,rgba(0,0,0,0.06))] px-3 py-1 text-[11px] uppercase tracking-[0.18em] text-muted-foreground">
        <span
          aria-hidden
          className="size-1.5 rounded-full bg-[var(--ant-color-primary,#0071e3)]"
        />
        {t("version_phase")}
      </span>
    </header>
  );
}
