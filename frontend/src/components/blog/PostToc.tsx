import { getTranslations } from "next-intl/server";

import type { PostHeading } from "@/lib/markdown";

const INDENT_BY_LEVEL: Record<PostHeading["level"], string> = {
  1: "pl-0",
  2: "pl-3",
  3: "pl-6",
};

/** Colonna sinistra del post (mockup 1a): "In questo post" con i titoli h1/h2/h3. */
export async function PostToc({ headings }: { headings: PostHeading[] }) {
  if (headings.length === 0) return null;
  const t = await getTranslations("PostPage");
  return (
    <nav aria-label={t("inThisPost")} className="flex flex-col gap-2 text-[13px]">
      <span className="font-mono text-[11px] uppercase tracking-[.08em] text-muted">{t("inThisPost")}</span>
      <ul className="flex flex-col gap-1.5 border-l border-border">
        {headings.map((h) => (
          <li key={h.id} className={INDENT_BY_LEVEL[h.level]}>
            <a href={`#${h.id}`} className="text-muted no-underline hover:text-foreground">
              {h.text}
            </a>
          </li>
        ))}
      </ul>
    </nav>
  );
}
