import { useTranslation } from "react-i18next";

interface TopbarProps {
  title: string;
  description?: string;
}

export function Topbar({ title, description }: TopbarProps) {
  const { t } = useTranslation("common");

  return (
    <header className="flex h-14 items-center justify-between border-b border-border px-6">
      <div>
        <h1 className="text-base font-semibold leading-apple-headline tracking-apple-tight">
          {title}
        </h1>
        {description ? (
          <p className="text-xs leading-snug text-muted-foreground">
            {description}
          </p>
        ) : null}
      </div>
      <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
        {t("version_phase")}
      </p>
    </header>
  );
}
