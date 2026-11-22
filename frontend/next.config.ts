import type { NextConfig } from "next";
import createNextIntlPlugin from "next-intl/plugin";

const withNextIntl = createNextIntlPlugin("./src/i18n/request.ts");

const nextConfig: NextConfig = {
  output: "standalone",
  experimental: {
    // CSS nell'HTML invece di <link> bloccanti: sono ~22 KB (Tailwind) e il pubblico di un
    // articolo arriva spesso da fuori, a cache fredda — vedi docs di Next "inlineCss".
    inlineCss: true,
  },
};

export default withNextIntl(nextConfig);
