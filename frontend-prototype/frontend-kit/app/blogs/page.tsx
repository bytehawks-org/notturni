import { getTranslations } from "next-intl/server";
import type { Metadata } from "next";
import { SiteHeader } from "@/components/shell/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { BlogDirectoryGrid } from "@/components/home/BlogDirectory";
import { FilterChip } from "@/components/ui/Pill";
import { getPublicBlogs } from "@/lib/server-api";
import Link from "next/link";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("BlogsPage"); const t = await getTranslations("BlogsPage"); return { title: t("title"), alternates: { canonical: "/blogs" } }; }

/** /blogs — mockup 4c. Only blogs with search_indexing_enabled and visibility=public are returned by the API. */
export default async function BlogsPage({ searchParams }: { searchParams: Promise<{ locale?: string; q?: string }> }) {
  const { locale, q } = await searchParams;
  const blogs = await getPublicBlogs({ limit: 60, locale, q, sort: "active" }).catch(() => []);
  const langs = [["", t("all")], ["it", "Italiano"], ["en", "English"], ["de", "Deutsch"], ["fr", "Français"]];
  return (
    <>
      <SiteHeader current="blogs" />
      <main className="mx-auto flex w-full max-w-[1184px] flex-col gap-7 px-5 py-10 lg:px-12 lg:py-12">
        <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
          <div className="flex flex-col gap-1.5"><h1 className="font-serif text-3xl font-medium tracking-tight md:text-[40px]">{t("title")}</h1><p className="text-muted md:text-base">{t("subtitle", { count: blogs.length })}</p></div>
          <form className="flex items-center gap-2 rounded-[10px] border border-border bg-surface px-3.5 py-2 text-sm text-muted md:w-80"><span aria-hidden="true">⌕</span><input name="q" defaultValue={q} placeholder={t("search")} className="w-full bg-transparent text-foreground outline-none placeholder:text-muted" /></form>
        </div>
        <div className="flex flex-wrap gap-2 border-b border-border pb-3">
          {langs.map(([code, label]) => <Link key={code} href={code ? `/blogs?locale=${code}` : "/blogs"}><FilterChip active={(locale ?? "") === code}>{label}</FilterChip></Link>)}
        </div>
        <BlogDirectoryGrid blogs={blogs} />
      </main>
      <SiteFooter />
    </>
  );
}
