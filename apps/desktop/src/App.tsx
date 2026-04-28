import { Button } from "@/components/ui/button";

export default function App() {
  return (
    <main className="flex min-h-screen items-center justify-center px-6">
      <section className="w-full max-w-2xl space-y-8 text-center">
        <header className="space-y-3">
          <p className="text-xs uppercase tracking-[0.18em] text-muted-foreground">
            v0.0.0 · Phase 1.0.1
          </p>
          <h1 className="font-display text-5xl font-semibold leading-apple-headline tracking-apple-tighter md:text-6xl">
            CCUse
          </h1>
          <p className="text-lg leading-snug tracking-apple-tight text-muted-foreground">
            本地 API 代理 + 多供应商无感切换
          </p>
        </header>

        <p className="text-sm leading-relaxed text-muted-foreground">
          脚手架已就绪：Tauri 2 · React 18 · Tailwind 3 · shadcn/ui。
          <br />
          后续功能将随 Phase 1.0.x 任务陆续接入。
        </p>

        <div className="flex items-center justify-center gap-3">
          <Button size="lg">开始配置</Button>
          <Button variant="pill" size="lg">
            查看文档
          </Button>
        </div>
      </section>
    </main>
  );
}
