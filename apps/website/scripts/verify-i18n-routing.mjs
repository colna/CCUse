import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.cwd();

function read(relativePath) {
  return readFileSync(path.join(root, relativePath), "utf8");
}

function readJson(relativePath) {
  return JSON.parse(read(relativePath));
}

function assertMissing(relativePath) {
  assert.equal(
    existsSync(path.join(root, relativePath)),
    false,
    `${relativePath} should not exist after locale routing moves pages under app/[locale]`,
  );
}

function detectLocale(acceptLanguage) {
  const languageRanges = acceptLanguage
    .split(",")
    .map((range) => {
      const [tag, quality = "q=1"] = range.trim().split(";");
      return {
        tag: tag.toLowerCase(),
        quality: Number(quality.replace("q=", "")),
      };
    })
    .filter(({ tag }) => tag.length > 0)
    .sort((left, right) => right.quality - left.quality);

  for (const { tag } of languageRanges) {
    if (tag === "zh" || tag.startsWith("zh-")) {
      return "zh";
    }

    if (tag === "en" || tag.startsWith("en-")) {
      return "en";
    }
  }

  return "en";
}

function expectedRedirectPath(pathname, acceptLanguage) {
  assert.equal(
    pathname,
    "/",
    "only root should rely on browser locale redirect",
  );
  return `/${detectLocale(acceptLanguage)}`;
}

const routing = read("i18n/routing.ts");
const requestConfig = read("i18n/request.ts");
const middleware = read("middleware.ts");
const layout = read("app/[locale]/layout.tsx");
const page = read("app/[locale]/page.tsx");
const en = readJson("messages/en.json");
const zh = readJson("messages/zh.json");

assertMissing("app/layout.tsx");
assertMissing("app/page.tsx");

assert.match(routing, /locales = \["zh", "en"\] as const/);
assert.match(routing, /defaultLocale = "en"/);
assert.match(routing, /localePrefix: "always"/);
assert.match(routing, /defineRouting/);
assert.match(requestConfig, /getRequestConfig/);
assert.match(requestConfig, /messages\/zh\.json/);
assert.match(requestConfig, /messages\/en\.json/);

assert.match(middleware, /next-intl\/middleware/);
assert.match(middleware, /createMiddleware\(routing\)/);
assert.match(middleware, /matcher: \[/);
assert.match(middleware, /api\|_next/);
assert.match(middleware, /\.\*\\\\\.\.\*/);

assert.match(layout, /generateStaticParams/);
assert.match(layout, /setRequestLocale\(locale\)/);
assert.match(layout, /<html lang=\{locale\}>/);
assert.match(layout, /NextIntlClientProvider/);
assert.match(page, /getTranslations\(\{ locale, namespace: "HomePage" \}\)/);

assert.equal(en.HomePage.actions.download, "Download");
assert.equal(zh.HomePage.actions.download, "下载");
assert.deepEqual(Object.keys(en.HomePage.capabilities), [
  "proxy",
  "routing",
  "failover",
]);
assert.deepEqual(
  Object.keys(en.HomePage.capabilities),
  Object.keys(zh.HomePage.capabilities),
);

assert.equal(expectedRedirectPath("/", "zh-CN,zh;q=0.9,en;q=0.8"), "/zh");
assert.equal(expectedRedirectPath("/", "en-US,en;q=0.9"), "/en");
assert.equal(expectedRedirectPath("/", "fr-FR,fr;q=0.9"), "/en");

console.log("Next.js i18n routing contract passed");
