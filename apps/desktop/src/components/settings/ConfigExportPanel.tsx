import { useCallback, useEffect, useState } from "react";
import {
  DownloadOutlined,
  ThunderboltFilled,
  UploadOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import {
  exportConfig,
  getTemplatePresets,
  importConfig,
  type TemplatePreset,
} from "@/lib/tauri";

/**
 * 设置页面里"导入 / 导出 / 模板预设"面板。
 *
 * - 导出走 `getTemplatePresets` 后端命令，得到一份对称加密的二进制
 *   `.ccuse`；密码由用户当场输入，前端不持久化。
 * - 导入是反向操作：选择文件 → 询问密码 → 调用后端命令。
 * - 模板预设当前只显示，不直接落库（点击后只是提示选中），等后端
 *   command 接入后再扩展。
 */

export function ConfigExportPanel() {
  const { t } = useTranslation("monitor");
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [status, setStatus] = useState<string | null>(null);
  const [statusIsError, setStatusIsError] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);

  useEffect(() => {
    // 后端无 Tauri 环境时（vitest jsdom）返回 reject，这里直接吞掉：
    // 预设是锦上添花，不该把整个 Settings 拉黑。
    getTemplatePresets()
      .then(setPresets)
      .catch(() => undefined);
  }, []);

  const handleExport = useCallback(async () => {
    const password = window.prompt(t("config_export_password_prompt"));
    if (!password) return;
    setExporting(true);
    setStatus(null);
    try {
      const blob = await exportConfig(password);
      downloadFile(blob, `ccuse-config-${Date.now()}.ccuse`);
      setStatus(t("config_export_success"));
      setStatusIsError(false);
    } catch (err) {
      setStatus(t("config_export_failed", { error: String(err) }));
      setStatusIsError(true);
    } finally {
      setExporting(false);
    }
  }, [t]);

  const handleImport = useCallback(() => {
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
        await importConfig(new Uint8Array(await file.arrayBuffer()), password);
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
  }, [t]);

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

/** 触发浏览器下载一段二进制；浏览器 / WebView 都通用。 */
function downloadFile(bytes: Uint8Array, filename: string) {
  const file = new Blob([bytes as BlobPart], {
    type: "application/octet-stream",
  });
  const url = URL.createObjectURL(file);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
