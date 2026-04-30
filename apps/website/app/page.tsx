import { Button } from "@ccuse/ui/button";
import { Card, CardContent } from "@ccuse/ui/card";

const capabilities = [
  "Local OpenAI-compatible proxy",
  "Multi-provider routing",
  "Automatic failover",
] as const;

export default function HomePage() {
  return (
    <main className="min-h-screen bg-background text-foreground">
      <section
        aria-labelledby="hero-title"
        className="mx-auto flex min-h-screen max-w-5xl flex-col justify-center px-6 py-24"
      >
        <p className="text-sm font-semibold text-primary">CCUse for desktop</p>
        <h1
          id="hero-title"
          className="mt-5 max-w-3xl font-display text-5xl font-semibold leading-apple-headline tracking-apple-tighter"
        >
          One local endpoint for resilient AI clients.
        </h1>
        <p className="mt-6 max-w-2xl text-lg leading-8 text-muted-foreground">
          Connect Cursor, Claude Desktop, and OpenAI-compatible tools to a
          loopback API that can route across providers and keep working when an
          upstream fails.
        </p>
        <nav aria-label="Primary actions" className="mt-8 flex gap-3">
          <Button asChild>
            <a href="/download">Download</a>
          </Button>
          <Button asChild variant="secondary">
            <a href="https://github.com/colna/CCUse">GitHub</a>
          </Button>
        </nav>
      </section>
      <section
        aria-labelledby="capabilities-title"
        className="mx-auto grid max-w-5xl gap-6 px-6 pb-24 sm:grid-cols-3"
      >
        <h2 id="capabilities-title" className="sr-only">
          Core capabilities
        </h2>
        <ul className="contents">
          {capabilities.map((item) => (
            <li key={item}>
              <Card className="shadow-apple-card">
                <CardContent className="p-5 text-sm font-medium">
                  {item}
                </CardContent>
              </Card>
            </li>
          ))}
        </ul>
      </section>
    </main>
  );
}
