import type { Post } from "./types";

/** Stato "visivo" di un post (mockup 1e/3g): `pending_review` → revisione,
 * `published` con data futura → pianificato. */
export type DisplayPostStatus = "draft" | "review" | "scheduled" | "published";

export function displayPostStatus(post: Pick<Post, "status" | "published_at">): DisplayPostStatus {
  if (post.status === "pending_review") return "review";
  if (post.status === "published") {
    return post.published_at && new Date(post.published_at).getTime() > Date.now() ? "scheduled" : "published";
  }
  return "draft";
}
