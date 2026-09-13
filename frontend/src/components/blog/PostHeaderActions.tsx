"use client";

import { useTranslations } from "next-intl";
import Link from "next/link";

import { FollowBlogButton } from "@/components/blog/FollowBlogButton";
import { ThemeToggle } from "@/components/ThemeToggle";
import { useAuth } from "@/lib/auth-context";

/** Azioni a destra nell'header pubblico del blog (mockup 1a/3f): tema, accesso/dashboard, segui. */
export function BlogHeaderActions({ slug }: { slug: string }) {
  const { user, loading } = useAuth();
  const t = useTranslations("Site");
  return (
    <>
      <span className="hidden sm:inline-flex">
        <ThemeToggle />
      </span>
      {!loading &&
        (user ? (
          <Link href="/dashboard" className="hidden text-muted no-underline hover:text-foreground sm:inline">
            {t("dashboard")}
          </Link>
        ) : (
          <Link href="/login" className="text-muted no-underline hover:text-foreground">
            {t("signIn")}
          </Link>
        ))}
      <FollowBlogButton slug={slug} />
    </>
  );
}
