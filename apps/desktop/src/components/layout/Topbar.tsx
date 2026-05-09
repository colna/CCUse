import { useTranslation } from "react-i18next";

interface TopbarProps {
  title: string;
  description?: string;
}

export function Topbar({ title, description }: TopbarProps) {
  const { t } = useTranslation("common");

  return (
    <header
      style={{
        background: "var(--ant-color-bg-container)",
        borderBottom: "1px solid var(--ant-color-border-secondary)",
        color: "var(--ant-color-text)",
      }}
      className="flex h-16 items-center justify-between px-8 backdrop-blur"
    >
      <div>
        <h1
          className="text-lg font-semibold leading-apple-headline tracking-apple-tight"
          style={{ color: "var(--ant-color-text)" }}
        >
          {title}
        </h1>
        {description ? (
          <p
            className="mt-0.5 text-xs leading-snug"
            style={{ color: "var(--ant-color-text-secondary)" }}
          >
            {description}
          </p>
        ) : null}
      </div>
      <span
        className="inline-flex items-center gap-2 rounded-full px-3 py-1 text-[11px] uppercase tracking-[0.18em]"
        style={{
          border: "1px solid var(--ant-color-border-secondary)",
          color: "var(--ant-color-text-tertiary)",
        }}
      >
        <span
          aria-hidden
          className="size-1.5 rounded-full"
          style={{ background: "var(--ant-color-primary)" }}
        />
        {t("version_phase")}
      </span>
    </header>
  );
}
