import { useCallback, useState } from "react";

import { AddProviderForm } from "@/components/providers/AddProviderForm";
import { ProviderList } from "@/components/providers/ProviderList";

export function ProvidersPage() {
  const [refreshKey, setRefreshKey] = useState(0);

  const handleAdded = useCallback(() => {
    setRefreshKey((k) => k + 1);
  }, []);

  return (
    <section className="mx-auto max-w-2xl space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        供应商
      </h2>
      <ProviderList refreshKey={refreshKey} />
      <AddProviderForm onAdded={handleAdded} />
    </section>
  );
}
