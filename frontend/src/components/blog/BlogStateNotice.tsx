import { getTranslations } from "next-intl/server";
import Link from "next/link";

import type { Blog } from "@/lib/types";

/** Mockup 3d "Blog suspended" / 5c "paused page": avviso al posto dei
 * contenuti quando un blog pubblico è sospeso da un admin o messo in pausa
 * dal proprietario (`GET /blogs/{slug}` risponde con i flag). */
export async function BlogStateNotice({ blog }: { blog: Blog }) {
  const t = await getTranslations("BlogState");
  const suspended = blog.is_suspended;
  return (
    <main className="mx-auto w-full max-w-[680px] flex-1 px-5 py-16">
      <div className="flex flex-col gap-3 rounded-xl border border-border bg-surface px-7 py-8">
        <span className={`font-mono text-[11px] uppercase tracking-[.08em] ${suspended ? "text-danger" : "text-muted"}`}>
          {suspended ? t("suspendedLabel") : t("pausedLabel")}
        </span>
        <h1 className="font-serif text-2xl text-foreground">{suspended ? t("suspendedTitle") : t("pausedTitle", { title: blog.title })}</h1>
        <p className="text-[15px] leading-relaxed text-muted">{suspended ? t("suspendedBody") : t("pausedBody")}</p>
        <Link href="/" className="text-sm font-medium text-primary no-underline hover:underline">
          {t("backHome")}
        </Link>
      </div>
    </main>
  );
}

export const blogIsOffline = (blog: Blog): boolean => blog.is_suspended || blog.is_paused;
