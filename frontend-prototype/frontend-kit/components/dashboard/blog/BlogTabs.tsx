import { useTranslations } from "next-intl";
import Link from "next/link";

export type BlogRole = "owner" | "co-author" | "author" | "reviewer" | "mediator";
type Tab = { key: string; roles: BlogRole[] };
const ALL: BlogRole[] = ["owner", "co-author", "author", "reviewer", "mediator"];
const TABS: Tab[] = [
  { key: "", roles: ALL },
  { key: "posts", roles: ["owner", "co-author", "author", "reviewer"] },
  { key: "pages", roles: ["owner", "co-author"] },
  { key: "publications", roles: ["owner", "co-author"] },
  { key: "taxonomy", roles: ["owner", "co-author"] },
  { key: "notes", roles: ["owner", "co-author", "author"] },
  { key: "comments", roles: ["owner", "mediator"] },
  { key: "collaborators", roles: ["owner"] },
  { key: "followers", roles: ["owner"] },
  { key: "appearance", roles: ["owner"] },
  { key: "settings", roles: ["owner"] },
];

export function BlogTabs({ slug, role, current, pendingComments = 0 }: { slug: string; role: BlogRole; current: string; pendingComments?: number }) {
  const t = useTranslations("BlogTabs");
  const base = `/dashboard/blogs/${slug}`;
  return (
    <nav className="-mx-5 flex gap-[22px] overflow-x-auto border-b border-border px-5 text-sm text-muted lg:mx-0 lg:px-0" aria-label="Blog sections">
      {TABS.filter((t_) => t_.roles.includes(role)).map((t_) => {
        const href = t_.key ? `${base}/${t_.key}` : base;
        const active = t_.key ? current.startsWith(href) : current === base;
        return (
          <Link key={t_.key} href={href} className={`-mb-px whitespace-nowrap border-b-2 pb-2.5 no-underline ${active ? "border-primary font-semibold text-foreground" : "border-transparent hover:text-foreground"}`}>
            {t(t_.key || "overview")}{t_.key === "comments" && pendingComments > 0 && <span className="ml-1 text-muted">· {pendingComments}</span>}
          </Link>
        );
      })}
    </nav>
  );
}
