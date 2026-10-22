"use client";
import { useTranslations } from "next-intl";
import Link from "next/link";
import { usePathname } from "next/navigation";
import type { ReactNode } from "react";
import { DashboardShell, AUTHOR_NAV } from "@/components/shell/DashboardShell";
import { BlogTabs, type BlogRole } from "@/components/dashboard/blog/BlogTabs";
import { Button } from "@/components/ui/Button";
import { VisibilityLabel, type Visibility } from "@/components/blog/VisibilityBand";

/** Layout for every blog-admin tab: header (avatar, title, slug, visibility, role) + role-gated tab strip. Mockups 5a–5c. */
export default function BlogAdminLayout({ children, params }: { children: ReactNode; params: { slug: string } }) {
  const path = usePathname();
  const t = useTranslations("BlogAdmin");
  // TODO replace with useBlog(params.slug) from lib/api
  const blog = { slug: params.slug, title: "Quaderno Notturno", visibility: "public" as Visibility, role: "owner" as BlogRole, pendingComments: 3 };
  return (
    <DashboardShell items={AUTHOR_NAV}>
      <div className="flex flex-col gap-3.5">
        <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
          <div className="flex items-center gap-3.5">
            <span className="grid h-12 w-12 place-items-center rounded-xl bg-primary font-serif text-[22px] text-background">{blog.title[0]}</span>
            <div className="flex flex-col gap-0.5">
              <h1 className="font-serif text-[22px] font-medium leading-none md:text-[28px]">{blog.title}</h1>
              <span className="font-mono text-[13px] text-muted">{blog.slug}.notturni.eu · <VisibilityLabel visibility={blog.visibility} /> · {t("youAre", { role: t(`roles.${blog.role}`) })}</span>
            </div>
          </div>
          <div className="flex gap-2.5"><Link href={`/${blog.slug}`} target="_blank"><Button variant="secondary">{t("viewBlog")}</Button></Link><Link href={`/dashboard/blogs/${blog.slug}/posts/new`}><Button>{t("writePost")}</Button></Link></div>
        </div>
        <BlogTabs slug={blog.slug} role={blog.role} current={path} pendingComments={blog.pendingComments} />
      </div>
      <div className="pt-6">{children}</div>
    </DashboardShell>
  );
}
