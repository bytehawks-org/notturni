import { useTranslations } from "next-intl";
import { Pill } from "../ui/Pill";

export type PostStatus = "draft" | "review" | "scheduled" | "published" | "hidden";
const MAP: Record<PostStatus, { tone: "neutral" | "warn" | "info" | "ok" | "danger"; label: string }> = {
  draft: { tone: "neutral", label: "draft" }, review: { tone: "warn", label: "review" }, scheduled: { tone: "info", label: "scheduled" },
  published: { tone: "ok", label: "published" }, hidden: { tone: "danger", label: "hidden" },
};
export function StatusPill({ status }: { status: PostStatus }) {
  const t = useTranslations("Status"); const m = MAP[status]; return <Pill tone={m.tone}>{t(m.label)}</Pill>; }
