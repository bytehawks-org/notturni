import { NextRequest, NextResponse } from "next/server";
import { SITE_HOST, SITE_URL } from "@/lib/site";

/** Stesso fallback di lib/api.ts::API_URL — non importato da lì per non
 * tirarsi dietro l'intero client API (tipi + funzioni) in un modulo che
 * gira ad ogni richiesta. */
const API_URL = (process.env.NEXT_PUBLIC_API_URL || "http://localhost:8000").replace(/\/$/, "");

/** Origini per connect-src, derivate dagli URL reali configurati invece che
 * hardcodate su un dominio fisso — altrimenti il dev locale (frontend su
 * localhost:3000, API su localhost:8000, origini diverse) verrebbe bloccato
 * dalla stessa CSP pensata per la produzione. Il wildcard sui sottodomini
 * serve alle pagine blog (`{slug}.{host}`) che chiamano l'API sull'apex. */
function connectSrcOrigins(): string {
  const site = new URL(SITE_URL);
  const api = new URL(API_URL);
  const origins = new Set([`${site.protocol}//${site.host}`, `${site.protocol}//*.${SITE_HOST}`, `${api.protocol}//${api.host}`]);
  return Array.from(origins).join(" ");
}

/** Sottodomini riservati alla piattaforma stessa, mai risolti come blog —
 * stessa lista di backend/app/domain/blog_rules.py::RESERVED_BLOG_SLUGS
 * (tenerle allineate: quella lista è la fonte di verità, applicata anche in
 * fase di creazione del blog). */
const RESERVED_SUBDOMAINS = new Set([
  "www",
  "blog",
  "mail",
  "journal",
  "api",
  "admin",
  "monitor",
  "stats",
  "status",
  "search",
  "users",
]);

/** Primo segmento dei path *di piattaforma* (non del blog) raggiungibili
 * anche da un sottodominio — es. `foo.notturni.eu/login`. I componenti
 * pubblici del blog generano link relativi come `/login`, `/dashboard`,
 * `/?category=...`: senza questo bypass la riscrittura sotto li porterebbe
 * su `/{slug}/login` ecc., che non è una route del blog (sotto
 * `frontend/src/app/[blogSlug]/` esistono solo `[postSlug]`, `bibliografia`,
 * `link`, `media`, `pagina`, `pub`, `search`, `atom.xml`, `feed.xml`) e produrrebbe un
 * 404 o il feed sbagliato invece della pagina di piattaforma attesa.
 * Tenerla allineata alle route dirette sotto `frontend/src/app/`. */
const PLATFORM_ONLY_PATHS = new Set([
  "login",
  "register",
  "forgot-password",
  "dashboard",
  "admin",
  "blogs",
  "users",
  "u",
  "p",
]);

/** Content-Security-Policy con nonce per-richiesta (CLAUDE.md #6, Traefik
 * come ingress): script-src 'self' senza nonce/unsafe-inline blocca anche
 * gli script inline che Next.js stesso inietta per l'idratazione (payload
 * RSC in streaming) — non solo script di terze parti. Prima di questo
 * blocco la CSP viveva in k8s/middleware-security-headers.yaml (statica,
 * un solo script-src per tutte le richieste): impediva la idratazione di
 * React su ogni pagina, login/registrazione inclusi (bottoni non
 * rispondevano ai click). Spostata qui per avere un nonce diverso ad ogni
 * richiesta — l'app è già interamente a rendering dinamico (next-intl legge
 * un cookie per la lingua in ogni pagina), quindi nessun impatto sul poter
 * generare pagine statiche che oggi non ci sono già. `'strict-dynamic'`
 * permette a uno script già fidato (col nonce) di caricare altri script
 * (es. Turnstile carica script propri) senza dover nonce-are ciascuno;
 * https://challenges.cloudflare.com resta comunque nell'allowlist per i
 * browser meno recenti che non supportano strict-dynamic. Gli altri header
 * di sicurezza (HSTS, X-Frame-Options, nosniff, Referrer-Policy) restano in
 * k8s/middleware-security-headers.yaml, statici — solo CSP è di proprietà
 * dell'app da qui in poi. */
function cspHeaderValue(nonce: string): string {
  const csp = `
    default-src 'self';
    img-src 'self' data: https:;
    style-src 'self' 'unsafe-inline';
    script-src 'self' 'nonce-${nonce}' 'strict-dynamic' https://challenges.cloudflare.com;
    frame-src https://challenges.cloudflare.com;
    font-src 'self' data:;
    connect-src 'self' ${connectSrcOrigins()} https://challenges.cloudflare.com;
    base-uri 'self';
    form-action 'self';
    frame-ancestors 'none';
  `;
  return csp.replace(/\s{2,}/g, " ").trim();
}

/** Routing per sottodominio-per-blog (CLAUDE.md #6, ROADMAP.md "Blog utente
 * su nomeutente.notturni.eu"): {slug}.notturni.eu viene riscritto
 * internamente su /{slug}/..., la stessa route path-based già servita da
 * frontend/src/app/[blogSlug]/. Riscrittura (rewrite), non redirect: l'URL
 * mostrato al visitatore resta il sottodominio, l'app instrada solo
 * internamente — coerente con l'IngressRoute Traefik dinamico
 * (k8s/ingressroute.yaml) che porta qui *tutto* il traffico su
 * *.notturni.eu. */
export function proxy(request: NextRequest) {
  const nonce = Buffer.from(crypto.randomUUID()).toString("base64");
  const csp = cspHeaderValue(nonce);

  const requestHeaders = new Headers(request.headers);
  requestHeaders.set("x-nonce", nonce);
  requestHeaders.set("Content-Security-Policy", csp);

  const host = request.headers.get("host") ?? "";
  const hostname = host.split(":")[0];

  const withCsp = (response: NextResponse) => {
    response.headers.set("Content-Security-Policy", csp);
    return response;
  };

  if (hostname === SITE_HOST || !hostname.endsWith(`.${SITE_HOST}`)) {
    return withCsp(NextResponse.next({ request: { headers: requestHeaders } }));
  }

  const subdomain = hostname.slice(0, -(SITE_HOST.length + 1));
  // Un'etichetta con un punto (es. un sottodominio "annidato" inatteso) o
  // riservata non va mai interpretata come slug di un blog.
  if (!subdomain || subdomain.includes(".") || RESERVED_SUBDOMAINS.has(subdomain)) {
    return withCsp(NextResponse.next({ request: { headers: requestHeaders } }));
  }

  const firstSegment = request.nextUrl.pathname.split("/")[1];
  if (PLATFORM_ONLY_PATHS.has(firstSegment)) {
    return withCsp(NextResponse.next({ request: { headers: requestHeaders } }));
  }

  const url = request.nextUrl.clone();
  url.pathname = `/${subdomain}${request.nextUrl.pathname}`;
  return withCsp(NextResponse.rewrite(url, { request: { headers: requestHeaders } }));
}

export const config = {
  matcher: ["/((?!_next/|api/|favicon.ico|robots.txt|sitemap.xml|theme-init.js).*)"],
};
