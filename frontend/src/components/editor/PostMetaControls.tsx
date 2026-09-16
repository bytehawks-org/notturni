"use client";

import { useTranslations } from "next-intl";

import { SegmentedControl } from "@/components/ui/Controls";
import { COMMENTS_MODES, type CommentsMode } from "@/lib/types";

function RailLabel({ children }: { children: React.ReactNode }) {
  return <span className="mb-1.5 block text-xs font-semibold uppercase tracking-[.04em] text-muted">{children}</span>;
}

// SegmentedControl vuole un value di tipo stringa: "inherit" rappresenta qui
// il `null` dell'API (eredita dal blog), tradotto ai margini di questi
// componenti — mai esposto al chiamante.
const INHERIT = "inherit" as const;

/** Override di Blog.comments_mode per il solo post corrente — stessa
 * semantica tri-state di PostCommentsModeSelect, ma come selettore a
 * segmenti (stile del controllo di stato) invece di un <select>. */
export function PostCommentsModeControl({
  value,
  onChange,
}: {
  value: CommentsMode | null;
  onChange: (value: CommentsMode | null) => void;
}) {
  const t = useTranslations("PostEditorPage");
  const options = [
    { value: INHERIT as string, label: t("commentsShort.inherit") },
    ...COMMENTS_MODES.map((m) => ({ value: m as string, label: t(`commentsShort.${m}`) })),
  ];
  return (
    <div>
      <RailLabel>{t("comments")}</RailLabel>
      <SegmentedControl
        value={value ?? INHERIT}
        options={options}
        onChange={(v) => onChange(v === INHERIT ? null : (v as CommentsMode))}
      />
    </div>
  );
}

export type NewPostStatus = "draft" | "review" | "published";

/** Stato scelto per un post non ancora creato (dashboard/blogs/[slug]/posts/new):
 * a differenza di PostStatusControl non compie transizioni via API (il post
 * non esiste ancora) — è solo la selezione che NewPostPage applica dopo la
 * creazione della bozza (submit-for-review/publish incatenati alla POST). */
export function NewPostStatusControl({ value, onChange }: { value: NewPostStatus; onChange: (v: NewPostStatus) => void }) {
  const t = useTranslations("Editor");
  const ts = useTranslations("Status");
  const options: { value: NewPostStatus; label: string }[] = [
    { value: "draft", label: ts("draft") },
    { value: "review", label: ts("review") },
    { value: "published", label: t("publishNow") },
  ];
  return (
    <div className="flex flex-col gap-2">
      <RailLabel>{t("status")}</RailLabel>
      <SegmentedControl value={value} options={options} onChange={onChange} />
    </div>
  );
}

/** Override di Blog.search_indexing_enabled/ai_crawling_enabled per il solo
 * post corrente (non può riaprire un crawler già escluso dal blog, solo
 * restringerlo ulteriormente — app/domain/seo.py), come selettore a segmenti. */
export function PostCrawlingControl({
  label,
  value,
  onChange,
}: {
  label: string;
  value: boolean | null;
  onChange: (value: boolean | null) => void;
}) {
  const t = useTranslations("PostEditorPage");
  const options = [
    { value: INHERIT as string, label: t("inheritBlog") },
    { value: "allow", label: t("allow") },
    { value: "block", label: t("block") },
  ];
  return (
    <div>
      <RailLabel>{label}</RailLabel>
      <SegmentedControl
        value={value === null ? INHERIT : value ? "allow" : "block"}
        options={options}
        onChange={(v) => onChange(v === INHERIT ? null : v === "allow")}
      />
    </div>
  );
}
