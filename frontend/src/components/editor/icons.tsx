import type { SVGProps } from "react";

/** Icone minime, inline (nessuna libreria esterna), 18x18, stroke corrente. */
function Icon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 18 18"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    />
  );
}

export function LinkIcon() {
  return (
    <Icon>
      <path d="M7.5 10.5a3 3 0 0 0 4.2.3l2-2a3 3 0 0 0-4.2-4.2l-1.1 1.1" />
      <path d="M10.5 7.5a3 3 0 0 0-4.2-.3l-2 2a3 3 0 0 0 4.2 4.2l1.1-1.1" />
    </Icon>
  );
}

export function QuoteIcon() {
  return (
    <Icon>
      <path d="M4 10.5V7.8A2.3 2.3 0 0 1 6.3 5.5" />
      <path d="M4 10.5h2.6v2.6H4z" />
      <path d="M10.4 10.5V7.8a2.3 2.3 0 0 1 2.3-2.3" />
      <path d="M10.4 10.5H13v2.6h-2.6z" />
    </Icon>
  );
}

export function BulletListIcon() {
  return (
    <Icon>
      <circle cx="4" cy="5" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="4" cy="9" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="4" cy="13" r="0.9" fill="currentColor" stroke="none" />
      <path d="M7.5 5h6.5" />
      <path d="M7.5 9h6.5" />
      <path d="M7.5 13h6.5" />
    </Icon>
  );
}

export function OrderedListIcon() {
  return (
    <Icon>
      <path d="M6.5 5h7" />
      <path d="M6.5 9h7" />
      <path d="M6.5 13h7" />
      <text x="1.5" y="6.3" fontSize="4.2" fill="currentColor" stroke="none">1</text>
      <text x="1.5" y="10.3" fontSize="4.2" fill="currentColor" stroke="none">2</text>
      <text x="1.5" y="14.3" fontSize="4.2" fill="currentColor" stroke="none">3</text>
    </Icon>
  );
}

export function ImageIcon() {
  return (
    <Icon>
      <rect x="2.5" y="3.5" width="13" height="11" rx="1.4" />
      <circle cx="6.3" cy="7.3" r="1.1" />
      <path d="M15.5 12.5 11.5 8.8 5 14.5" />
    </Icon>
  );
}

export function UndoIcon() {
  return (
    <Icon>
      <path d="M4.5 8H11a3.3 3.3 0 0 1 0 6.5H7.5" />
      <path d="M4.5 8 7.3 5.2" />
      <path d="M4.5 8 7.3 10.8" />
    </Icon>
  );
}

export function RedoIcon() {
  return (
    <Icon>
      <path d="M13.5 8H7a3.3 3.3 0 0 0 0 6.5h3.5" />
      <path d="M13.5 8 10.7 5.2" />
      <path d="M13.5 8 10.7 10.8" />
    </Icon>
  );
}

export function TableIcon() {
  return (
    <Icon>
      <rect x="2.5" y="3.5" width="13" height="11" rx="1.2" />
      <path d="M2.5 7.5h13M2.5 11.5h13M7 3.5v11" />
    </Icon>
  );
}
