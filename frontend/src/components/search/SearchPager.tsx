import Link from "next/link";

interface SearchPagerProps {
  page: number;
  hasNext: boolean;
  hrefFor: (page: number) => string;
  labels: { previous: string; next: string; page: string; nav: string };
}

/** Precedente / successiva con link reali: funziona senza JavaScript lato client. */
export function SearchPager({ page, hasNext, hrefFor, labels }: SearchPagerProps) {
  const linkClass =
    "flex min-h-11 items-center rounded-md px-2 text-primary no-underline hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary";
  return (
    <nav aria-label={labels.nav} className="flex items-center justify-between border-t border-border pt-4 text-sm">
      {page > 1 ? (
        <Link href={hrefFor(page - 1)} className={linkClass}>
          {labels.previous}
        </Link>
      ) : (
        <span />
      )}
      <span className="text-muted">{labels.page}</span>
      {hasNext ? (
        <Link href={hrefFor(page + 1)} className={linkClass}>
          {labels.next}
        </Link>
      ) : (
        <span />
      )}
    </nav>
  );
}
