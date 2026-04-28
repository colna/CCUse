import { AddProviderForm } from "@/components/providers/AddProviderForm";

export function ProvidersPage() {
  return (
    <section className="mx-auto max-w-2xl space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        供应商
      </h2>
      <AddProviderForm />
    </section>
  );
}
