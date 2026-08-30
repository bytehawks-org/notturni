type Kind = "error" | "success" | "info";

const KIND_CLASSES: Record<Kind, string> = {
  error: "border-red-700/30 bg-red-700/10 text-red-800",
  success: "border-primary/30 bg-primary/10 text-primary",
  info: "border-border bg-foreground/5 text-foreground",
};

export function Alert({ kind = "info", children }: { kind?: Kind; children: React.ReactNode }) {
  return <div className={`rounded-md border px-3 py-2 text-sm ${KIND_CLASSES[kind]}`}>{children}</div>;
}
