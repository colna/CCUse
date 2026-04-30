import { existsSync, rmSync } from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const root = process.cwd();
const site = path.join(root, ".next/server/app");
const outputPath = path.join(root, "public/_pagefind");

if (!existsSync(site)) {
  throw new Error(
    "Pagefind source .next/server/app is missing. Run next build first.",
  );
}

rmSync(outputPath, { force: true, recursive: true });

const result = spawnSync(
  "pnpm",
  [
    "exec",
    "pagefind",
    "--site",
    site,
    "--output-path",
    outputPath,
    "--root-selector",
    "[data-pagefind-body]",
    "--glob",
    "**/*.html",
    "--quiet",
  ],
  {
    cwd: root,
    stdio: "inherit",
  },
);

if (result.status !== 0) {
  throw new Error(
    `Pagefind index build failed with exit code ${result.status}`,
  );
}
