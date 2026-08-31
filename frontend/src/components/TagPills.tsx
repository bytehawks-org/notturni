import Link from "next/link";

/** Tag di un post, cliccabili: rimandano al filtro per tag della homepage.
 * `label`, se passato (es. "Tag:"), precede l'elenco. */
export function TagPills({ tags, label }: { tags: string[]; label?: string }) {
  if (tags.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-2">
      {label && <span className="text-sm text-muted">{label}</span>}
      {tags.map((tag) => (
        <Link
          key={tag}
          href={`/?tag=${encodeURIComponent(tag)}`}
          className="rounded-full bg-primary/10 px-3 py-1 text-xs text-primary hover:bg-primary/20"
        >
          #{tag}
        </Link>
      ))}
    </div>
  );
}
