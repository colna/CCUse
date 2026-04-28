import { useCallback, useEffect, useState } from "react";
import { Download, Upload, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  exportConfig,
  importConfig,
  getTemplatePresets,
  type TemplatePreset,
} from "@/lib/tauri";

/** Config export / import / template presets panel (T1.0.4.18-20). */
export function ConfigExportPanel() {
  const [presets, setPresets] = useState<TemplatePreset[]>([]);
  const [status, setStatus] = useState<string | null>(null);
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
    const password = window.prompt("Enter a password to encrypt the export:");
    if (!password) return;
    setExporting(true);
    setStatus(null);
    try {
      const blob = await exportConfig(password);
      const file = new Blob([blob], { type: "application/octet-stream" });
      const url = URL.createObjectURL(file);
      const a = document.createElement("a");
      a.href = url;
      a.download = `ccuse-config-${Date.now()}.ccuse`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      setStatus("Export successful");
    } catch (err) {
      setStatus(`Export failed: ${String(err)}`);
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
      const password = window.prompt(
        "Enter the password used to encrypt this file:",
      );
      if (!password) return;
      setImporting(true);
      setStatus(null);
      try {
        const buffer = await file.arrayBuffer();
        await importConfig(new Uint8Array(buffer), password);
        setStatus("Import successful — providers and settings applied");
      } catch (err) {
        setStatus(`Import failed: ${String(err)}`);
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
          Config Export / Import
        </h3>
        <p className="text-xs text-muted-foreground">
          Export your provider configuration as an encrypted file, or import a
          previously exported configuration. API keys are not included in
          exports.
        </p>
      </div>

      <div className="flex flex-wrap gap-3">
        <Button
          variant="outline"
          size="sm"
          disabled={exporting}
          onClick={handleExport}
        >
          <Download className="size-3.5" />
          {exporting ? "Exporting..." : "Export Config"}
        </Button>
        <Button
          variant="outline"
          size="sm"
          disabled={importing}
          onClick={handleImport}
        >
          <Upload className="size-3.5" />
          {importing ? "Importing..." : "Import Config"}
        </Button>
      </div>

      {status && (
        <p
          className={`text-xs ${status.startsWith("Export failed") || status.startsWith("Import failed") ? "text-destructive" : "text-green-500"}`}
        >
          {status}
        </p>
      )}

      {presets.length > 0 && (
        <div className="space-y-3">
          <h4 className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
            Quick-Start Templates
          </h4>
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
            {presets.map((preset) => (
              <button
                key={preset.id}
                className="group flex flex-col gap-1.5 rounded-lg border border-border p-4 text-left transition-colors hover:border-primary/50 hover:bg-muted/30"
                onClick={() => {
                  setStatus(
                    `Template "${preset.name}" selected — add your API key in the Providers tab to activate`,
                  );
                }}
              >
                <div className="flex items-center gap-2">
                  <Sparkles className="size-4 text-primary/60 group-hover:text-primary" />
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
