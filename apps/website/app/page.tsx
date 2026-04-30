const capabilities = [
  "Local OpenAI-compatible proxy",
  "Multi-provider routing",
  "Automatic failover",
] as const;

export default function HomePage() {
  return (
    <main>
      <section aria-labelledby="hero-title">
        <p>CCUse for desktop</p>
        <h1 id="hero-title">One local endpoint for resilient AI clients.</h1>
        <p>
          Connect Cursor, Claude Desktop, and OpenAI-compatible tools to a
          loopback API that can route across providers and keep working when an
          upstream fails.
        </p>
        <nav aria-label="Primary actions">
          <a href="/download">Download</a>
          <a href="https://github.com/colna/CCUse">GitHub</a>
        </nav>
      </section>
      <section aria-labelledby="capabilities-title">
        <h2 id="capabilities-title">Core capabilities</h2>
        <ul>
          {capabilities.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      </section>
    </main>
  );
}
