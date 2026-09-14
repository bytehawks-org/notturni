import { type InputHTMLAttributes, type TextareaHTMLAttributes, type ReactNode, forwardRef } from "react";

const base = "w-full rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground placeholder:text-muted focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20";

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input({ className = "", ...p }, ref) {
  return <input ref={ref} className={`${base} ${className}`} {...p} />;
});
export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function TextArea({ className = "", ...p }, ref) {
  return <textarea ref={ref} className={`${base} min-h-32 ${className}`} {...p} />;
});
export function Label({ children, htmlFor, hint }: { children: ReactNode; htmlFor?: string; hint?: ReactNode }) {
  return (
    <label htmlFor={htmlFor} className="mb-1.5 flex justify-between text-xs font-semibold uppercase tracking-[.04em] text-muted">
      <span>{children}</span>{hint && <span className="font-normal normal-case tracking-normal">{hint}</span>}
    </label>
  );
}
export function FieldGroup({ children }: { children: ReactNode }) { return <div className="flex flex-col">{children}</div>; }
