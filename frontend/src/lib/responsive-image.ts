/** Varianti responsive delle immagini di contenuto (generate all'upload dal backend,
 * `crates/noct-media/src/variants.rs`: le due regole vanno tenute allineate).
 *
 * Nessun elenco da interrogare: le dimensioni dell'originale stanno nel nome del file
 * (`{uuid}-{W}x{H}.{ext}`) e le varianti WebP sono `{uuid}-{W}x{H}-{w}.webp`, con le larghezze `w`
 * ricalcolate qui. Le immagini senza dimensioni nel nome (precedenti a questa funzione, GIF,
 * immagini di altri siti) non hanno varianti: `responsiveImage` restituisce `null`. */

export const MAX_BASE_WIDTH = 2560;
export const MIN_VARIANT_WIDTH = 320;

/** Larghezze delle varianti di un originale largo `width`, dalla più grande: 100% (limitato a
 * `MAX_BASE_WIDTH`), 75%, 50%, 25%, scartando quelle sotto `MIN_VARIANT_WIDTH`. */
export function variantWidths(width: number): number[] {
  const base = Math.min(width, MAX_BASE_WIDTH);
  const out = [base];
  for (const [num, den] of [[3, 4], [1, 2], [1, 4]]) {
    const w = Math.floor((base * num) / den);
    if (w >= MIN_VARIANT_WIDTH && !out.includes(w)) out.push(w);
  }
  return out;
}

export interface ResponsiveImage {
  width: number;
  height: number;
  /** Valore dell'attributo `srcset`, solo varianti WebP (`<w>w`). */
  srcSet: string;
}

// uuid + dimensioni + estensione, in un oggetto dello storage utente: un URL di un altro sito che
// per caso finisca in `-800x600.jpg` non ha varianti e non deve ricevere un srcset rotto
const NAME_RE = /^(.*\/userdata\/.+\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})-(\d{1,5})x(\d{1,5})\.(?:png|jpe?g|webp)$/i;

export function responsiveImage(url: string): ResponsiveImage | null {
  const clean = url.split(/[?#]/)[0];
  const m = NAME_RE.exec(clean);
  if (!m) return null;
  const [, prefix, w, h] = m;
  const width = Number(w);
  const height = Number(h);
  if (!width || !height) return null;
  const stem = `${prefix}-${width}x${height}`;
  const srcSet = variantWidths(width)
    .map((vw) => `${stem}-${vw}.webp ${vw}w`)
    .join(", ");
  return { width, height, srcSet };
}
