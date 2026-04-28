import { useCallback, useEffect, useState } from "react";
import { Pencil, Plus, Trash2, Check, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  getModelMappings,
  setModelMapping,
  removeModelMapping,
  type MappingEntry,
} from "@/lib/tauri";

const VENDORS = ["openai", "anthropic", "gemini"] as const;
type Vendor = (typeof VENDORS)[number];

/** Editable model mapping table (T1.0.3.12). */
export function ModelMappingTable() {
  const { t } = useTranslation("monitor");
  const { t: tc } = useTranslation("common");
  const [entries, setEntries] = useState<MappingEntry[]>([]);
  const [editing, setEditing] = useState<{
    model: string;
    vendor: Vendor;
    value: string;
  } | null>(null);
  const [adding, setAdding] = useState(false);
  const [newRow, setNewRow] = useState({
    client_model: "",
    openai: "",
    anthropic: "",
    gemini: "",
  });

  const load = useCallback(async () => {
    try {
      const data = await getModelMappings();
      setEntries(data);
    } catch {
      // Tauri not available in dev/test -- use empty list.
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const handleEdit = async () => {
    if (!editing) return;
    if (editing.value.trim()) {
      await setModelMapping(
        editing.model,
        editing.vendor,
        editing.value.trim(),
      );
    } else {
      await removeModelMapping(editing.model, editing.vendor);
    }
    setEditing(null);
    load();
  };

  const handleAddRow = async () => {
    if (!newRow.client_model.trim()) return;
    const promises: Promise<void>[] = [];
    for (const v of VENDORS) {
      const val = newRow[v].trim();
      if (val) {
        promises.push(setModelMapping(newRow.client_model.trim(), v, val));
      }
    }
    await Promise.all(promises);
    setAdding(false);
    setNewRow({ client_model: "", openai: "", anthropic: "", gemini: "" });
    load();
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-foreground">
          {t("model_mapping_title")}
        </h3>
        <button
          onClick={() => setAdding(true)}
          className="flex items-center gap-1.5 rounded-md bg-primary/10 px-3 py-1.5 text-xs font-medium text-primary transition-colors hover:bg-primary/20"
          aria-label={t("model_mapping_add_aria")}
        >
          <Plus className="size-3.5" />
          {t("model_mapping_add")}
        </button>
      </div>

      <div className="overflow-x-auto rounded-lg border border-border">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border bg-muted/40">
              <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">
                {t("model_mapping_client_model")}
              </th>
              <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">
                OpenAI
              </th>
              <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">
                Anthropic
              </th>
              <th className="px-4 py-2.5 text-left font-medium text-muted-foreground">
                Gemini
              </th>
            </tr>
          </thead>
          <tbody>
            {entries.map((entry) => (
              <tr
                key={entry.client_model}
                className="border-b border-border/50 last:border-0"
              >
                <td className="px-4 py-2 font-mono text-xs text-foreground">
                  {entry.client_model}
                </td>
                {VENDORS.map((vendor) => {
                  const val = entry[vendor];
                  const isEditing =
                    editing?.model === entry.client_model &&
                    editing?.vendor === vendor;

                  if (isEditing) {
                    return (
                      <td key={vendor} className="px-4 py-1.5">
                        <div className="flex items-center gap-1">
                          <input
                            type="text"
                            value={editing.value}
                            onChange={(e) =>
                              setEditing({ ...editing, value: e.target.value })
                            }
                            onKeyDown={(e) => {
                              if (e.key === "Enter") handleEdit();
                              if (e.key === "Escape") setEditing(null);
                            }}
                            className="w-full rounded border border-border bg-background px-2 py-1 font-mono text-xs"
                            // eslint-disable-next-line jsx-a11y/no-autofocus
                            autoFocus
                          />
                          <button
                            onClick={handleEdit}
                            className="text-primary hover:text-primary/80"
                            aria-label={t("model_mapping_confirm_aria")}
                          >
                            <Check className="size-3.5" />
                          </button>
                          <button
                            onClick={() => setEditing(null)}
                            className="text-muted-foreground hover:text-foreground"
                            aria-label={t("model_mapping_cancel_aria")}
                          >
                            <X className="size-3.5" />
                          </button>
                        </div>
                      </td>
                    );
                  }

                  return (
                    <td key={vendor} className="group px-4 py-2">
                      <div className="flex items-center gap-2">
                        <span className="font-mono text-xs text-foreground/70">
                          {val ?? "--"}
                        </span>
                        <button
                          onClick={() =>
                            setEditing({
                              model: entry.client_model,
                              vendor,
                              value: val ?? "",
                            })
                          }
                          className="invisible text-muted-foreground hover:text-primary group-hover:visible"
                          aria-label={t("model_mapping_edit_aria", {
                            model: entry.client_model,
                            vendor,
                          })}
                        >
                          <Pencil className="size-3" />
                        </button>
                        {val && (
                          <button
                            onClick={async () => {
                              await removeModelMapping(
                                entry.client_model,
                                vendor,
                              );
                              load();
                            }}
                            className="invisible text-muted-foreground hover:text-destructive group-hover:visible"
                            aria-label={t("model_mapping_delete_aria", {
                              model: entry.client_model,
                              vendor,
                            })}
                          >
                            <Trash2 className="size-3" />
                          </button>
                        )}
                      </div>
                    </td>
                  );
                })}
              </tr>
            ))}

            {adding && (
              <tr className="border-b border-border/50 bg-muted/20">
                <td className="px-4 py-1.5">
                  <input
                    type="text"
                    placeholder="model name"
                    value={newRow.client_model}
                    onChange={(e) =>
                      setNewRow({ ...newRow, client_model: e.target.value })
                    }
                    className="w-full rounded border border-border bg-background px-2 py-1 font-mono text-xs"
                    // eslint-disable-next-line jsx-a11y/no-autofocus
                    autoFocus
                  />
                </td>
                {VENDORS.map((v) => (
                  <td key={v} className="px-4 py-1.5">
                    <input
                      type="text"
                      placeholder={v}
                      value={newRow[v]}
                      onChange={(e) =>
                        setNewRow({ ...newRow, [v]: e.target.value })
                      }
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleAddRow();
                        if (e.key === "Escape") setAdding(false);
                      }}
                      className="w-full rounded border border-border bg-background px-2 py-1 font-mono text-xs"
                    />
                  </td>
                ))}
              </tr>
            )}

            {adding && (
              <tr>
                <td colSpan={4} className="px-4 py-2">
                  <div className="flex gap-2">
                    <button
                      onClick={handleAddRow}
                      className="rounded-md bg-primary px-3 py-1 text-xs font-medium text-primary-foreground"
                    >
                      {t("model_mapping_save")}
                    </button>
                    <button
                      onClick={() => setAdding(false)}
                      className="rounded-md bg-muted px-3 py-1 text-xs font-medium text-muted-foreground"
                    >
                      {tc("cancel")}
                    </button>
                  </div>
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <p className="text-xs text-muted-foreground">{t("model_mapping_hint")}</p>
    </div>
  );
}
