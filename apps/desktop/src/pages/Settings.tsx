import { ModelMappingTable } from "@/components/model-mapping/ModelMappingTable";
import { ConfigExportPanel } from "@/components/settings/ConfigExportPanel";

export function SettingsPage() {
  return (
    <section className="space-y-8">
      <div className="space-y-3">
        <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
          设置
        </h2>
        <p className="text-sm leading-relaxed text-muted-foreground">
          应用偏好（端口 / 主题 / 启动选项）将在 T1.0.4 落地。
        </p>
      </div>

      <ModelMappingTable />

      <hr className="border-border" />

      <ConfigExportPanel />
    </section>
  );
}
