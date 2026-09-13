// Append to src/lib/types.ts — fields the new home/directory components read.
export interface TrendingTag { tag: string; post_count: number }

export interface PublicBlog {
  slug: string; title: string; subtitle?: string | null; description?: string | null;
  avatar_url?: string | null; languages?: string[]; post_count: number; follower_count?: number | null;
}

// Optional additions to Post (all nullable; components degrade gracefully when absent):
//   blog_title?: string; author_avatar_url?: string | null; reading_minutes?: number | null; footnote_count?: number | null;
//
// server-api.ts: add
//   export const getPublicBlogs = (q: { limit?: number; locale?: string; q?: string; sort?: "active" | "followers" | "new" }) =>
//     get<PublicBlog[]>("/api/v1/blogs", { public: 1, indexed: 1, ...q });
