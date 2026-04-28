import { LocalApiCard } from "@/components/local-api/LocalApiCard";

export function DashboardPage() {
  return (
    <section className="space-y-6">
      <h2 className="text-2xl font-semibold leading-apple-headline tracking-apple-tight">
        总览
      </h2>
      <LocalApiCard />
    </section>
  );
}
