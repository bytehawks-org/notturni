import { FeedPostCard } from "@/components/FeedPostCard";
import { SiteHeader } from "@/components/SiteHeader";
import { getPublicFeed } from "@/lib/server-api";

export default async function Home() {
  const posts = await getPublicFeed({ limit: 20 }).catch(() => []);

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

        <div className="mx-auto mt-16 max-w-2xl">
          {posts.length === 0 ? (
            <p className="text-center text-sm text-muted">Ancora nessun post pubblicato.</p>
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
