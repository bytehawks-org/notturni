import { type ButtonHTMLAttributes, forwardRef } from "react";

type Variant = "primary" | "secondary" | "danger" | "ghost";
type Size = "sm" | "md" | "lg";
const V: Record<Variant, string> = {
  primary: "bg-primary text-background font-semibold hover:opacity-90",
  secondary: "border border-border text-foreground font-medium hover:bg-foreground/5",
  danger: "bg-danger text-white font-semibold hover:opacity-90",
  ghost: "text-muted hover:text-foreground",
};
const S: Record<Size, string> = { sm: "px-3 py-1.5 text-[13px] rounded-[7px]", md: "px-3.5 py-2 text-sm rounded-lg", lg: "px-4 py-3.5 text-base rounded-xl w-full" };

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> { variant?: Variant; size?: Size }

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button({ variant = "primary", size = "md", className = "", ...props }, ref) {
  return <button ref={ref} className={`inline-flex items-center justify-center gap-2 transition disabled:cursor-not-allowed disabled:opacity-50 ${V[variant]} ${S[size]} ${className}`} {...props} />;
});
