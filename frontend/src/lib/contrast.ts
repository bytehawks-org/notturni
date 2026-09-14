/** Rapporto di contrasto WCAG 2.x tra due colori esadecimali (mockup 2e,
 * "Check contrast · AA"). AA: ≥ 4.5 per il testo, ≥ 3 per elementi grafici. */
function channel(v: number): number {
  const c = v / 255;
  return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

export function hexToRgb(hex: string): [number, number, number] | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(hex.trim());
  if (!m) return null;
  let h = m[1];
  if (h.length === 3) h = h.split("").map((c) => c + c).join("");
  const n = parseInt(h, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

export function luminance(hex: string): number | null {
  const rgb = hexToRgb(hex);
  if (!rgb) return null;
  const [r, g, b] = rgb;
  return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

export function contrastRatio(a: string, b: string): number | null {
  const la = luminance(a);
  const lb = luminance(b);
  if (la === null || lb === null) return null;
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

function rgbToHex(r: number, g: number, b: number): string {
  const c = (v: number) => Math.max(0, Math.min(255, Math.round(v))).toString(16).padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`;
}

function mix(hex: string, target: [number, number, number], amount: number): string {
  const rgb = hexToRgb(hex);
  if (!rgb) return hex;
  return rgbToHex(
    rgb[0] + (target[0] - rgb[0]) * amount,
    rgb[1] + (target[1] - rgb[1]) * amount,
    rgb[2] + (target[2] - rgb[2]) * amount
  );
}

/** Variante scura derivata dalla palette chiara (mockup 2e "Genera variante
 * scura"): sfondo e bordo verso il nero, testo e attenuato verso il bianco,
 * primario schiarito quanto basta per restare leggibile su fondo scuro. */
export function deriveDarkPalette(light: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  if (light.background) out.background = mix(light.background, [16, 16, 18], 0.9);
  if (light.foreground) out.foreground = mix(light.foreground, [240, 236, 228], 0.88);
  if (light.primary) out.primary = mix(light.primary, [255, 255, 255], 0.35);
  if (light.muted) out.muted = mix(light.muted, [255, 255, 255], 0.2);
  if (light.border) out.border = mix(light.border, [24, 24, 26], 0.85);
  return out;
}
