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
}: {
  src: string;
  alt?: string;
  sensitive: boolean;
  className: string;
  revealLabel: string;
  sensitiveLabel: string;
  expandLabel?: string;
}) {
  if (!sensitive) {
    // eslint-disable-next-line @next/next/no-img-element -- URL storage esterno
    return <img src={src} alt={alt} data-lightbox="1" className={className} />;
  }
  return (
    <label className={`sensitive-image-wrapper ${className}`}>
      <input type="checkbox" className="sensitive-image-toggle" aria-label={revealLabel} />
      {/* eslint-disable-next-line @next/next/no-img-element -- URL storage esterno */}
      <img src={src} alt={alt} className="h-full w-full object-cover" />
      <span className="sensitive-image-overlay">{sensitiveLabel}</span>
      <button type="button" className="lightbox-expand-btn" data-lightbox-src={src} data-lightbox-alt={alt} aria-label={expandLabel}>
        <ExpandIcon />
      </button>
    </label>
  );
}
