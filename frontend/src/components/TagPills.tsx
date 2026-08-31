import Link from "next/link";

/** Tag di un post, cliccabili: rimandano al filtro per tag della homepage. */
export function TagPills({ tags }: { tags: string[] }) {
  if (tags.length === 0) return null;
  return (
    <div className="flex flex-wrap gap-2">
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
