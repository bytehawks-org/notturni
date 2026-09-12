import { type InputHTMLAttributes, type TextareaHTMLAttributes, type ReactNode, forwardRef } from "react";

function baseFieldClasses(className: string) {
  return `w-full rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground placeholder:text-muted focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20 ${className}`;
}

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function Input(
  { className = "", ...props },
  ref
) {
  return <input ref={ref} className={baseFieldClasses(className)} {...props} />;
});

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(
  function TextArea({ className = "", ...props }, ref) {
    return <textarea ref={ref} className={baseFieldClasses(`min-h-32 ${className}`)} {...props} />;
  }
);

export function Label({ children, htmlFor, hint }: { children: ReactNode; htmlFor?: string; hint?: ReactNode }) {
  return (
    <label htmlFor={htmlFor} className="mb-1.5 flex justify-between text-xs font-semibold uppercase tracking-[.04em] text-muted">
      <span>{children}</span>
      {hint && <span className="font-normal normal-case tracking-normal">{hint}</span>}
    </label>
  );
}

// mb-4 preservato oltre a flex flex-col: gli usi esistenti di <FieldGroup>
// nel resto dell'app si affidano a questo margine per lo spazio verticale
// tra campi consecutivi (il kit lo delega a un contenitore con gap, non
// ancora presente nelle pagine non ancora restilizzate).
export function FieldGroup({ children, className = "" }: { children: ReactNode; className?: string }) {
  return <div className={`mb-4 flex flex-col ${className}`}>{children}</div>;
}
