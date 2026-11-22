import { FillImage } from "@/components/FillImage";
import { ExpandIcon } from "@/components/editor/icons";

/**
 * Un'immagine di copertina (blog o post) pubblica, con lo stesso trucco CSS
 * label→checkbox delle immagini sensibili nel corpo del post (vedi
 * lib/markdown.ts::wrapSensitiveImages) e il relativo pulsante di
 * ingrandimento per la Lightbox (Rifinitura #1, components/Lightbox.tsx) —
 * mai un <div> al posto del <label> (il click non arriverebbe al checkbox).
 * Nessun JS qui: il click sul pulsante è intercettato dalla delega globale
 * di LightboxProvider via `.lightbox-expand-btn`/`data-lightbox-src`, quindi
 * questo resta un componente server-safe (usabile da Server Component).
 */
export function SensitiveImage({
  src,
  alt = "",
  sensitive,
  className,
  revealLabel,
  sensitiveLabel,
  expandLabel = "Ingrandisci",
  priority = false,
  sizes = "(min-width: 768px) 1184px, 100vw",
}: {
  src: string;
  alt?: string;
  sensitive: boolean;
  className: string;
  revealLabel: string;
  sensitiveLabel: string;
  expandLabel?: string;
  /** Copertina nella prima schermata (LCP): caricamento immediato e priorità alta. */
  priority?: boolean;
  /** Larghezza di rendering per il browser, che sceglie la variante dal `srcset`. */
  sizes?: string;
}) {
  if (!sensitive) {
    return (
      <div className={`relative ${className}`}>
        <FillImage
          src={src}
          alt={alt}
          data-lightbox="1"
          tabIndex={0}
          role="button"
          // Con `alt` vuoto (copertina senza testo alternativo) il controllo
          // da tastiera resterebbe senza nome accessibile — riusa la stessa
          // etichetta del pulsante di ingrandimento del ramo sensibile qui
          // sotto (bug segnalato dalla review Copilot).
          aria-label={alt || expandLabel}
          sizes={sizes}
          priority={priority}
          className="object-cover"
        />
      </div>
    );
  }
  return (
    <label className={`sensitive-image-wrapper ${className}`}>
      <input type="checkbox" className="sensitive-image-toggle" aria-label={revealLabel} />
      <FillImage src={src} alt={alt} sizes={sizes} priority={priority} className="object-cover" />
      <span className="sensitive-image-overlay">{sensitiveLabel}</span>
      <button type="button" className="lightbox-expand-btn" data-lightbox-src={src} data-lightbox-alt={alt} aria-label={expandLabel}>
        <ExpandIcon />
      </button>
    </label>
  );
}
