import { PlusOutlined, UpOutlined } from "@ant-design/icons";
import { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { AddProviderForm } from "@/components/providers/AddProviderForm";
import { ProviderList } from "@/components/providers/ProviderList";

/**
 * 供应商管理页：
 * - 列表 `ProviderList` 自带轮询健康快照；
 * - 新增表单 `AddProviderForm` 折叠在按钮后，避免空状态时表单占满视口。
 *
 * `refreshKey` 是给列表组件的"显式重新拉取"信号；每次添加成功后自增，
 * 触发 list 重新 fetch；这样不需要在 list 中订阅 add 事件、也不需要
 * 把 providers state 上提到页面。
 */

const ADD_PROVIDER_FORM_PANEL_ID = "add-provider-form-panel";

export function ProvidersPage() {
  const [refreshKey, setRefreshKey] = useState(0);
  const [addFormOpen, setAddFormOpen] = useState(false);
  const { t } = useTranslation("providers");

  const handleAdded = useCallback(() => {
    setRefreshKey((k) => k + 1);
    setAddFormOpen(false);
  }, []);

  const toggleAddForm = useCallback(() => {
    setAddFormOpen((open) => !open);
  }, []);

  return (
    <section className="mx-auto max-w-2xl space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        {t("title")}
      </h2>
      <ProviderList refreshKey={refreshKey} />
      <div className="flex justify-end">
        <Button
          htmlType="button"
          size="middle"
          type={addFormOpen ? "default" : "primary"}
          onClick={toggleAddForm}
          aria-expanded={addFormOpen}
          aria-controls={ADD_PROVIDER_FORM_PANEL_ID}
          icon={
            addFormOpen ? (
              <UpOutlined aria-label="" role="presentation" />
            ) : (
              <PlusOutlined aria-label="" role="presentation" />
            )
          }
        >
          {addFormOpen
            ? t("collapse_add_provider_form")
            : t("add_provider_title")}
        </Button>
      </div>
      {addFormOpen ? (
        <div id={ADD_PROVIDER_FORM_PANEL_ID}>
          <AddProviderForm onAdded={handleAdded} />
        </div>
      ) : null}
    </section>
  );
}
