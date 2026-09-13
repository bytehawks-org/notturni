import { useTranslations } from "next-intl";
import Link from "next/link";

export interface MediaItem { id: string; src: string; alt: string; post?: { title: string; href: string }; date?: string; sensitive?: boolean; badge?: "cover" | "no-alt" | "unused" }

/** Public /{blog}/media and the author library share this grid; `editable` adds badges. 2 cols mobile → 4/5 desktop. */
export function MediaGrid({ items, editable = false, onOpen }: { items: MediaItem[]; editable?: boolean; onOpen?: (m: MediaItem) => void }) {
  const t = useTranslations("Media");
  return (
    <div className={`grid grid-cols-2 gap-3 md:gap-5 ${editable ? "lg:grid-cols-5" : "lg:grid-cols-4"}`}>
      {items.map((m) => (
        <figure key={m.id} className="flex min-w-0 flex-col gap-2">
          <button type="button" onClick={() => onOpen?.(m)} className="relative block aspect-square overflow-hidden rounded-[10px] border border-border bg-surface md:aspect-[4/3]">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src={m.src} alt={m.alt} className={`h-full w-full object-cover transition ${m.sensitive ? "blur-xl" : ""}`} />
            {editable && m.badge && <Badge kind={m.badge} />}
            {editable && m.sensitive && <span className="absolute left-2 top-2 rounded bg-sensitive px-1.5 py-0.5 text-[11px] font-semibold text-white">{t("sensitive")}</span>}
          </button>
          <figcaption className="flex flex-col gap-0.5 text-[13px] leading-snug">
            <span className="truncate font-medium">{m.alt || <span className="text-danger">{t("missingAlt")}</span>}</span>
            {m.post && <span className="truncate text-muted">{t("in")} <Link href={m.post.href}>{m.post.title}</Link>{m.date && ` · ${m.date}`}</span>}
            {!editable && m.sensitive && <span className="text-xs text-sensitive">{t("sensitiveReveal")}</span>}
          </figcaption>
        </figure>
      ))}
    </div>
  );
}
function Badge({ kind }: { kind: NonNullable<MediaItem["badge"]> }) {
  const cls = { cover: "bg-primary text-background", "no-alt": "bg-danger text-white", unused: "bg-muted text-background" }[kind];
  return <span className={`absolute left-2 top-2 rounded px-1.5 py-0.5 text-[11px] font-semibold ${cls}`}>{t(`badge.${kind}`)}</span>;
}
