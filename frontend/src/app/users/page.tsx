import type { Metadata } from "next";
import { getLocale, getTranslations } from "next-intl/server";
import Link from "next/link";

import { UserDirectoryGrid } from "@/components/home/UserDirectory";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { FilterChip } from "@/components/ui/Pill";
import { interestLabel } from "@/lib/interests";
import { languageName } from "@/lib/languages";
import { getPublicInterests, getPublicUsers } from "@/lib/server-api";

export async function generateMetadata(): Promise<Metadata> {
  const t = await getTranslations("UsersPage");
  return { title: t("title"), alternates: { canonical: "/users" } };
}

const DIRECTORY_LOCALES = ["it", "en", "de", "fr"];

/** Directory pubblica degli utenti (CLAUDE.md #5, "directory di utenti" per
 * poterli cercare/seguire): stesso schema di `/blogs`, `q`/`locale` passati
 * a `GET /api/v1/users`. Esclude già lato backend gli utenti che hanno
 * scelto l'opt-out dalla directory (`dashboard/profile`, sezione Privacy).
 * `interest` filtra per chiave canonica (blocco "interessi utente"),
 * raggiunto anche dai chip cliccabili sul profilo pubblico. */
export default async function UsersPage({
  searchParams,
}: {
  searchParams: Promise<{ locale?: string; q?: string; interest?: string }>;
}) {
  const { locale, q, interest } = await searchParams;
  const [users, allInterests, uiLocale, t] = await Promise.all([
    getPublicUsers({
      limit: 100,
      q: q?.trim() || undefined,
      locale: locale || undefined,
      interest: interest || undefined,
      sort: "new",
    }).catch(() => []),
    getPublicInterests(),
    getLocale(),
    getTranslations("UsersPage"),
  ]);
  const href = (code: string) => {
    const params = new URLSearchParams();
    if (q) params.set("q", q);
    if (interest) params.set("interest", interest);
    if (code) params.set("locale", code);
    const qs = params.toString();
    return qs ? `/users?${qs}` : "/users";
  };
  const activeInterest = allInterests.find((i) => i.key === interest);
  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-[1184px] flex-1 flex-col gap-7 px-5 py-10 lg:px-12 lg:py-12">
        <div className="flex flex-col gap-4 md:flex-row md:items-end md:justify-between">
          <div className="flex flex-col gap-1.5">
            <h1 className="font-serif text-3xl font-medium tracking-tight md:text-[40px]">{t("title")}</h1>
            <p className="text-muted md:text-base">{t("subtitle", { count: users.length })}</p>
          </div>
          <form className="flex items-center gap-2 rounded-[10px] border border-border bg-surface px-3.5 py-2 text-sm text-muted md:w-80">
            {locale && <input type="hidden" name="locale" value={locale} />}
            {interest && <input type="hidden" name="interest" value={interest} />}
            <span aria-hidden="true">⌕</span>
            <input
              name="q"
              defaultValue={q}
              placeholder={t("search")}
              aria-label={t("search")}
              className="w-full bg-transparent text-foreground outline-none placeholder:text-muted"
            />
          </form>
        </div>
        {interest && (
          <div className="flex items-center gap-2.5 text-sm text-muted">
            <span>
              {t("filteredByInterest")}{" "}
              <b className="font-semibold text-foreground">{interestLabel(activeInterest, interest, uiLocale)}</b>
            </span>
            <Link href={href("")} className="text-[13px] no-underline hover:underline">
              {t("removeFilter")}
            </Link>
          </div>
        )}
        <div className="flex flex-wrap gap-2 border-b border-border pb-3">
          <Link href={href("")} className="no-underline">
            <FilterChip active={!locale}>{t("all")}</FilterChip>
          </Link>
          {DIRECTORY_LOCALES.map((code) => (
            <Link key={code} href={href(code)} className="no-underline">
              <FilterChip active={locale === code}>{languageName(code, code)}</FilterChip>
            </Link>
          ))}
        </div>
        <UserDirectoryGrid users={users} interests={allInterests} locale={uiLocale} />
      </main>
      <SiteFooter />
    </>
  );
}
