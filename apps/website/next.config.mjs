import createNextIntlPlugin from "next-intl/plugin";

/** @type {import('next').NextConfig} */
const nextConfig = {
  poweredByHeader: false,
  reactStrictMode: true,
  transpilePackages: ["@ccuse/ui"],
};

const withNextIntl = createNextIntlPlugin();

export default withNextIntl(nextConfig);
