import { useCallback, useState } from "react";
import { UpOutlined, PlusOutlined } from "@ant-design/icons";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { AddProviderForm } from "@/components/providers/AddProviderForm";
import { ProviderList } from "@/components/providers/ProviderList";

const ADD_PROVIDER_FORM_PANEL_ID = "add-provider-form-panel";

export function ProvidersPage() {
  const [refreshKey, setRefreshKey] = useState(0);
  const [addFormOpen, setAddFormOpen] = useState(false);
  const { t } = useTranslation("providers");

  const handleAdded = useCallback(() => {
    setRefreshKey((k) => k + 1);
    setAddFormOpen(false);
  }, []);

  const handleToggleAddForm = useCallback(() => {
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
          onClick={handleToggleAddForm}
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
