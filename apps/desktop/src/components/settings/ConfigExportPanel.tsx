import { useCallback, useEffect, useState } from "react";
import {
  DownloadOutlined,
  UploadOutlined,
  ThunderboltFilled,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import {
  exportConfig,
  importConfig,
  getTemplatePresets,
  type TemplatePreset,
} from "@/lib/tauri";

/** Config export / import / template presets panel (T1.0.4.18-20). */
export function ConfigExportPanel() {
  const { t } = useTranslation("monitor");
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [status, setStatus] = useState<string | null>(null);
  const [statusIsError, setStatusIsError] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);

  const loadPresets = useCallback(async () => {
    try {
      const data = await getTemplatePresets();
      setPresets(data);
    } catch {
      // Tauri not available in dev/test
    }
  }, []);

  useEffect(() => {
    loadPresets();
  }, [loadPresets]);

  const handleExport = async () => {
    const password = window.prompt(t("config_export_password_prompt"));
    if (!password) return;
    setExporting(true);
    setStatus(null);
    try {
      const blob = await exportConfig(password);
      const file = new Blob([blob as BlobPart], {
        type: "application/octet-stream",
      });
      const url = URL.createObjectURL(file);
      const a = document.createElement("a");
      a.href = url;
      a.download = `ccuse-config-${Date.now()}.ccuse`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setStatus(t("config_export_success"));
      setStatusIsError(false);
    } catch (err) {
      setStatus(t("config_export_failed", { error: String(err) }));
      setStatusIsError(true);
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".ccuse";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      const password = window.prompt(t("config_import_password_prompt"));
      if (!password) return;
      setImporting(true);
      setStatus(null);
      try {
        const buffer = await file.arrayBuffer();
        await importConfig(new Uint8Array(buffer), password);
        setStatus(t("config_import_success"));
        setStatusIsError(false);
      } catch (err) {
        setStatus(t("config_import_failed", { error: String(err) }));
        setStatusIsError(true);
      } finally {
        setImporting(false);
      }
    };
    input.click();
  };

  return (
    <div className="space-y-6">
      <div className="space-y-1">
        <h3 className="text-sm font-medium text-foreground">
          {t("config_export_title")}
        </h3>
        <p className="text-xs text-muted-foreground">
          {t("config_export_desc")}
        </p>
      </div>

      <div className="flex flex-wrap gap-3">
        <Button
          type="default"
          disabled={exporting}
          onClick={handleExport}
          icon={<DownloadOutlined aria-label="" role="presentation" />}
        >
          {exporting ? t("config_exporting") : t("config_export_btn")}
        </Button>
        <Button
          type="default"
          disabled={importing}
          onClick={handleImport}
          icon={<UploadOutlined aria-label="" role="presentation" />}
        >
          {importing ? t("config_importing") : t("config_import_btn")}
        </Button>
      </div>

      {status && (
        <p
          className={`text-xs ${statusIsError ? "text-destructive" : "text-green-500"}`}
        >
          {status}
        </p>
      )}

      {presets.length > 0 && (
        <div className="space-y-3">
          <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            {t("config_templates_title")}
          </h4>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            {presets.map((preset) => (
              <button
                key={preset.id}
                className="hover:border-[var(--app-primary)]/50 group flex flex-col gap-1.5 rounded-2xl border border-[var(--app-border-secondary)] bg-[var(--app-bg-container)] p-4 text-left transition-colors"
                onClick={() => {
                  setStatus(
                    t("config_template_selected", { name: preset.name }),
                  );
                  setStatusIsError(false);
                }}
              >
                <div className="flex items-center gap-2">
                  <ThunderboltFilled
                    className="text-[var(--app-primary)]/60 group-hover:text-[var(--app-primary)]"
                    aria-label=""
                    role="presentation"
                  />
                  <span className="text-sm font-medium text-foreground">
                    {preset.name}
                  </span>
                </div>
                <p className="text-xs text-muted-foreground">
                  {preset.description}
                </p>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
