import { useTranslations } from "next-intl";
import Link from "next/link";

/** Superscript reference; `title` gives the native tooltip, matching lib/markdown.ts output. */
export function FootnoteRef({ n, text }: { n: number; text: string }) {
  const t = useTranslations("Post");
  return <sup className="footnote-ref" title={text}><a href={`#nota-${n}`} id={`ref-${n}`} className="no-underline">{n}</a></sup>;
}

export function Footnotes({ notes, bibliographyHref }: { notes: { n: number; text: string }[]; bibliographyHref?: string }) {
  const t = useTranslations("Post");
  if (!notes.length) return null;
  return (
    <section className="footnotes">
      <div className="flex items-baseline justify-between"><span className="footnotes-title">{t("notes")}</span>{bibliographyHref && <Link href={bibliographyHref} className="text-[13px]">{t("blogBibliography")}</Link>}</div>
      <ol className="flex flex-col gap-2 pl-5">
        {notes.map((f) => <li key={f.n} id={`nota-${f.n}`}>{f.text} <a href={`#ref-${f.n}`} className="footnote-backref ml-1.5" aria-label={t("backToText")}>↩</a></li>)}
      </ol>
    </section>
  );
}
