/** Dati strutturati Schema.org (JSON-LD): `<script type="application/ld+json">`
 * inline, letto dai crawler ma invisibile ai lettori. `<` viene escapato per
 * sicurezza — il payload contiene testo scritto dagli autori (titolo,
 * estratto), mai HTML, ma previene comunque una chiusura anticipata del tag
 * se contenesse `</script>`. */
export function JsonLd({ data }: { data: Record<string, unknown> }) {
  const json = JSON.stringify(data).replace(/</g, "\\u003c");
  return <script type="application/ld+json" dangerouslySetInnerHTML={{ __html: json }} />;
}
