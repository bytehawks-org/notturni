import type { Metadata, ResolvingMetadata } from "next";

type OpenGraph = NonNullable<Metadata["openGraph"]>;

/** Open Graph di una pagina arricchito con quello del portale. Next unisce i metadata dei segmenti in modo
 * *superficiale*: una pagina che definisce `openGraph` sostituisce per intero quello del layout e perderebbe nome
 * del sito, lingua e immagine di condivisione. Qui si riprendono dal layout (`parent`) ciò che la pagina non
 * dichiara: l'immagine del portale fa da ripiego quando la pagina non ne ha una propria. */
export async function withSiteOpenGraph(parent: ResolvingMetadata, og: OpenGraph): Promise<OpenGraph> {
  const site = (await parent).openGraph;
  return {
    ...og,
    siteName: og.siteName ?? site?.siteName ?? undefined,
    locale: og.locale ?? site?.locale ?? undefined,
    images: og.images ?? site?.images ?? undefined,
  } as OpenGraph;
}
