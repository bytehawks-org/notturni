"use client";

import { useEffect, useState } from "react";

import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";

import { errorMessage } from "./shared";

export function MyMembershipCard({ blogSlug }: { blogSlug: string }) {
  const { authFetch } = useAuth();
  const [alias, setAlias] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    authFetch((token) => api.blogs.memberOf(token))
      .then((list) => {
        const mine = list.find((m) => m.blog.slug === blogSlug);
        setAlias(mine ? (mine.author_display_name ?? "") : null);
      })
      .catch(() => setAlias(null));
  }, [authFetch, blogSlug]);

  if (alias === null) return null;

  async function handleSave() {
    setError(null);
    try {
      const updated = await authFetch((token) =>
        api.blogs.updateMyMembership(token, blogSlug, alias ?? "")
      );
      setAlias(updated.author_display_name ?? "");
      setSaved(true);
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  return (
    <Card>
      <CardTitle>Il mio nome su questo blog</CardTitle>
      <FieldGroup>
        <Label htmlFor="my-alias">Alias autore (todo/BLOG.md #4)</Label>
        <Input
          id="my-alias"
          value={alias}
          maxLength={255}
          placeholder="Lasciare vuoto per usare il nome predefinito del blog o il tuo alias di profilo"
          onChange={(e) => {
            setAlias(e.target.value);
            setSaved(false);
          }}
        />
        <p className="mt-1 text-xs text-muted">
          Con cui firmi i post scritti qui. Ha la precedenza sul nome predefinito del blog e
          sull&apos;alias del tuo profilo.
        </p>
      </FieldGroup>
      {error && (
        <div className="mb-3">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {saved && (
        <div className="mb-3">
          <Alert kind="success">Salvato.</Alert>
        </div>
      )}
      <Button onClick={handleSave}>Salva</Button>
    </Card>
  );
}
