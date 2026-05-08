import { useCallback, useState } from "react";
import { ChevronUp, Plus } from "lucide-react";
import { useTranslation } from "react-i18next";

import { AddProviderForm } from "@/components/providers/AddProviderForm";
import { ProviderList } from "@/components/providers/ProviderList";
import { Button } from "@/components/ui/button";

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
          type="button"
          size="sm"
          variant={addFormOpen ? "outline" : "default"}
          onClick={handleToggleAddForm}
          aria-expanded={addFormOpen}
          aria-controls={ADD_PROVIDER_FORM_PANEL_ID}
        >
          {addFormOpen ? (
            <ChevronUp className="mr-2 size-3.5" />
          ) : (
            <Plus className="mr-2 size-3.5" />
          )}
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
