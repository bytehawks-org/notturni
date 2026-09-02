import Link from "next/link";

import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/SiteHeader";
import { getPublicFeed, getTrendingTags } from "@/lib/server-api";

export default async function Home({
  searchParams,
}: {
  searchParams: Promise<{ tag?: string; category?: string }>;
}) {
  const { tag, category } = await searchParams;
  const [posts, trending] = await Promise.all([
    getPublicFeed({ limit: 20, tag, category }).catch(() => []),
    getTrendingTags({ days: 7, limit: 8 }).catch(() => []),
  ]);

  return (
    <>
      <SiteHeader />
      <main className="flex-1 px-6 py-16">
        <div className="mx-auto max-w-xl space-y-6 text-center">
          <h1 className="font-serif text-4xl sm:text-5xl tracking-tight">Notturni</h1>
          <p className="text-lg text-muted leading-relaxed">
            Permettere all&apos;utente di esprimersi nella forma più naturale possibile: la
            parola. Nella sicurezza e nella ricchezza della propria lingua.
          </p>
        </div>

        {trending.length > 0 && (
          <div className="mx-auto mt-12 max-w-2xl">
            <p className="mb-3 text-xs uppercase tracking-wide text-muted">Di tendenza questa settimana</p>
            <div className="flex flex-wrap gap-2">
              {trending.map((t) => (
                <Link
                  key={t.tag}
                  href={`/?tag=${encodeURIComponent(t.tag)}`}
                  className={`rounded-full px-3 py-1 text-sm transition ${
                    tag === t.tag
                      ? "bg-primary text-background"
                      : "bg-primary/10 text-primary hover:bg-primary/20"
                  }`}
                >
                  #{t.tag} <span className="opacity-70">· {t.post_count}</span>
                </Link>
              ))}
            </div>
          </div>
        )}

        <div className="mx-auto mt-16 max-w-2xl">
          {(tag || category) && (
            <div className="mb-6 flex items-center gap-2 text-sm text-muted">
              <span>
                {tag && (
                  <>
                    Post con il tag <span className="text-foreground">#{tag}</span>
                  </>
                )}
                {category && (
                  <>
                    Post nella categoria <span className="text-foreground">{category}</span>
                  </>
                )}
              </span>
              <Link href="/" className="text-primary hover:underline">
                Rimuovi filtro
              </Link>
            </div>
          )}
          {posts.length === 0 ? (
            <p className="text-center text-sm text-muted">
              {tag || category ? "Nessun post trovato." : "Ancora nessun post pubblicato."}
            </p>
          ) : (
            <div>
              {posts.map((post) => (
                <FeedPostCard key={post.id} post={post} />
              ))}
            </div>
          )}
        </div>
      </main>
    </>
  );
}
