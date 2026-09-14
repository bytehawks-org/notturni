"use client";

import { useTranslations } from "next-intl";
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
  const t = useTranslations("MyMembershipCard");
  const tc = useTranslations("Common");
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
      setError(errorMessage(err, tc("unexpectedError")));
    }
  }

  return (
    <Card>
      <CardTitle>{t("title")}</CardTitle>
      <FieldGroup>
        <Label htmlFor="my-alias">{t("label")}</Label>
        <Input
          id="my-alias"
          value={alias}
          maxLength={255}
          placeholder={t("placeholder")}
          onChange={(e) => {
            setAlias(e.target.value);
            setSaved(false);
          }}
        />
        <p className="mt-1 text-xs text-muted">{t("hint")}</p>
      </FieldGroup>
      {error && (
        <div className="mb-3">
          <Alert kind="error">{error}</Alert>
        </div>
      )}
      {saved && (
        <div className="mb-3">
          <Alert kind="success">{t("saved")}</Alert>
        </div>
      )}
      <Button onClick={handleSave}>{tc("save")}</Button>
    </Card>
  );
}
