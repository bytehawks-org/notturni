"use client";

import Link from "next/link";
import { useCallback, useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { type Blog, type Page } from "@/lib/types";

import { errorMessage } from "./shared";

export function PagesTab({ blog, canWrite }: { blog: Blog; canWrite: boolean }) {
  const { accessToken, authFetch } = useAuth();
  const [pages, setPages] = useState<Page[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(() => {
    api.blogs
      .listPages(blog.slug, blog.default_locale, accessToken)
      .then(setPages)
      .catch((err) => setError(errorMessage(err)));
  }, [blog.slug, blog.default_locale, accessToken]);

  useEffect(load, [load]);

  async function handlePublish(pageId: string) {
    try {
      const updated = await authFetch((token) =>
        api.blogs.updatePage(token, blog.slug, pageId, { is_published: true })
      );
      setPages((prev) => prev?.map((p) => (p.id === pageId ? updated : p)) ?? null);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  if (!blog.static_pages_enabled) {
    return (
      <p className="text-sm text-muted">
        Le pagine statiche non sono attive per questo blog: puoi attivarle dalla scheda
        Impostazioni.
      </p>
    );
  }

  return (
    <div>
      {canWrite && (
        <div className="mb-4">
          <Link href={`/dashboard/blogs/${blog.slug}/pages/new`}>
            <Button>Nuova pagina</Button>
          </Link>
        </div>
      )}
      {error && <Alert kind="error">{error}</Alert>}
      {pages !== null && pages.length === 0 && <p className="text-sm text-muted">Nessuna pagina.</p>}
      <div className="space-y-3">
        {pages?.map((page) => (
          <Card key={page.id} className="flex items-center justify-between">
            <div>
              <Link
                href={`/dashboard/blogs/${blog.slug}/pages/${page.id}`}
                className="font-serif text-lg text-foreground hover:text-primary"
              >
                {page.title}
              </Link>
              <p className="text-sm text-muted">
                /{page.slug} · {page.locale} · {page.is_published ? "pubblicata" : "bozza"}
              </p>
            </div>
            <div className="flex items-center gap-3">
              {page.is_published && page.permalink && (
                <Link href={page.permalink} className="text-sm text-primary hover:underline">
                  Vedi
                </Link>
              )}
              {canWrite && !page.is_published && (
                <Button variant="secondary" onClick={() => handlePublish(page.id)}>
                  Pubblica
                </Button>
              )}
            </div>
          </Card>
        ))}
      </div>
    </div>
  );
}
