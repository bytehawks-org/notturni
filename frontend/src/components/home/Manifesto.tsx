import { getTranslations } from "next-intl/server";
import Link from "next/link";

import { Button } from "@/components/ui/Button";

const PILLARS = ["language", "noAi", "notes", "portability"] as const;

/** Hero della home di piattaforma (mockup 4a/4b). Il manifesto è il testo
 * del progetto (CLAUDE.md §5), riportato tale e quale. */
export async function Manifesto() {
  const t = await getTranslations("Home");
  return (
    <section className="border-b border-border bg-surface">
      <div className="mx-auto grid w-full max-w-[1184px] items-end gap-10 px-5 py-12 lg:grid-cols-[minmax(0,1.2fr)_minmax(0,1fr)] lg:gap-16 lg:px-12 lg:py-[72px]">
        <div className="flex flex-col gap-5">
          <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("eyebrow")}</span>
          <h1 className="font-serif text-[34px] font-medium leading-[1.1] tracking-[-.015em] text-pretty md:text-[56px] md:leading-[1.08]">
            {t("manifestoTitle")}
          </h1>
          <p className="max-w-[560px] text-lg leading-relaxed text-muted text-pretty md:text-xl">{t("manifestoBody")}</p>
          <div className="mt-1.5 flex flex-wrap items-center gap-2.5">
            <Link href="/register">
              <Button className="px-5 py-2.5 text-[15px]">{t("startBlog")}</Button>
            </Link>
            <Link href="/blogs">
              <Button variant="secondary" className="px-5 py-2.5 text-[15px]">
                {t("browseBlogs")}
              </Button>
            </Link>
            <span className="ml-2 hidden text-[13px] text-muted md:inline">{t("limits")}</span>
          </div>
        </div>
        <div className="hidden grid-cols-2 gap-3.5 text-sm lg:grid">
          {PILLARS.map((k) => (
            <div key={k} className="flex flex-col gap-1.5 rounded-xl border border-border bg-background px-[18px] py-4">
              <span className="font-serif text-[17px]">{t(`pillars.${k}.title`)}</span>
              <span className="text-[13px] leading-relaxed text-muted">{t(`pillars.${k}.body`)}</span>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
