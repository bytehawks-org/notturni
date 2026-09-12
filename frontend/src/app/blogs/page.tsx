import type { Metadata } from "next";

import { BlogDirectoryGrid } from "@/components/home/BlogDirectory";
import { SiteHeader } from "@/components/SiteHeader";
import { SiteFooter } from "@/components/shell/SiteFooter";
import { getPublicBlogs } from "@/lib/server-api";

export const metadata: Metadata = {
  title: "Blog",
  alternates: { canonical: "/blogs" },
};

/** Directory pubblica dei blog indicizzabili (todo/UX_REDESIGN.md, mockup 4c). */
export default async function BlogsPage() {
  const blogs = await getPublicBlogs({ limit: 60 }).catch(() => []);
  return (
    <>
      <SiteHeader />
      <main className="mx-auto flex w-full max-w-[1184px] flex-col gap-7 px-5 py-10 lg:px-12 lg:py-12">
        <div className="flex flex-col gap-1.5">
          <h1 className="font-serif text-3xl font-medium tracking-tight md:text-[40px]">Blog</h1>
          <p className="text-muted md:text-base">
            {blogs.length === 0 ? "Ancora nessun blog pubblico." : `${blogs.length} blog pubblici`}
          </p>
        </div>
        <BlogDirectoryGrid blogs={blogs} />
      </main>
      <SiteFooter />
    </>
  );
}
