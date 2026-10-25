import type { ImgHTMLAttributes } from "react";

import { responsiveImage } from "@/lib/responsive-image";

/** Immagine che riempie il contenitore (`position: relative`), al posto di `next/image` con
 * `fill unoptimized`, che non emette `srcset`. Usa le varianti WebP generate all'upload quando
 * l'URL le ha (vedi lib/responsive-image.ts), altrimenti è un normale `<img>`. `priority` per
 * l'immagine nella prima schermata (LCP): niente lazy loading e priorità alta. */
export function FillImage({
  src,
  alt = "",
  sizes,
  priority = false,
  className = "",
  ...rest
}: Omit<ImgHTMLAttributes<HTMLImageElement>, "src" | "srcSet" | "loading"> & {
  src: string;
  sizes: string;
  priority?: boolean;
}) {
  const responsive = responsiveImage(src);
  return (
    // eslint-disable-next-line @next/next/no-img-element -- srcset da varianti già generate, nessuna ottimizzazione runtime
    <img
      src={src}
      alt={alt}
      srcSet={responsive?.srcSet}
      sizes={responsive ? sizes : undefined}
      loading={priority ? "eager" : "lazy"}
      fetchPriority={priority ? "high" : undefined}
      decoding="async"
      className={`absolute inset-0 h-full w-full ${className}`}
      {...rest}
    />
  );
}
