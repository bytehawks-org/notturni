import { getTranslations } from "next-intl/server";
import Link from "next/link";

import type { TrendingTag } from "@/lib/types";

/** "Di tendenza questa settimana" (mockup 4a/1c): stessi dati di `getTrendingTags`. */
export async function TrendingTags({ tags, active }: { tags: TrendingTag[]; active?: string }) {
  if (!tags.length) return null;
  const t = await getTranslations("Home");
  return (
    <div className="flex flex-col gap-3">
      <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("trending")}</span>
      <div className="flex gap-2 overflow-x-auto md:flex-wrap">
        {tags.map((tag) => (
          <Link
            key={tag.tag}
            href={`/?tag=${encodeURIComponent(tag.tag)}`}
            className={`whitespace-nowrap rounded-full px-[11px] py-[5px] text-[13px] no-underline transition ${
              active === tag.tag ? "bg-primary text-background" : "bg-primary/10 text-primary hover:bg-primary/20"
            }`}
          >
            #{tag.tag} <span className="opacity-70">· {tag.post_count}</span>
          </Link>
        ))}
      </div>
    </div>
  );
}
