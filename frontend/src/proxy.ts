import { NextRequest, NextResponse } from "next/server";
import { SITE_HOST } from "@/lib/site";

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
]);

/** Primo segmento dei path *di piattaforma* (non del blog) raggiungibili
 * anche da un sottodominio — es. `foo.notturni.eu/login`. I componenti
 * pubblici del blog generano link relativi come `/login`, `/dashboard`,
 * `/?category=...`: senza questo bypass la riscrittura sotto li porterebbe
 * su `/{slug}/login` ecc., che non è una route del blog (sotto
 * `frontend/src/app/[blogSlug]/` esistono solo `[postSlug]`, `bibliografia`,
 * `link`, `media`, `pagina`, `pub`, `atom.xml`, `feed.xml`) e produrrebbe un
 * 404 o il feed sbagliato invece della pagina di piattaforma attesa.
 * Tenerla allineata alle route dirette sotto `frontend/src/app/`. */
const PLATFORM_ONLY_PATHS = new Set([
  "login",
  "register",
  "forgot-password",
  "dashboard",
  "admin",
  "blogs",
  "u",
  "p",
]);

/** Routing per sottodominio-per-blog (CLAUDE.md #6, ROADMAP.md "Blog utente
 * su nomeutente.notturni.eu"): {slug}.notturni.eu viene riscritto
 * internamente su /{slug}/..., la stessa route path-based già servita da
 * frontend/src/app/[blogSlug]/. Riscrittura (rewrite), non redirect: l'URL
 * mostrato al visitatore resta il sottodominio, l'app instrada solo
 * internamente — coerente con l'IngressRoute Traefik dinamico (k8s/ingressroute.yaml)
 * che porta qui *tutto* il traffico su *.notturni.eu. */
export function proxy(request: NextRequest) {
  const host = request.headers.get("host") ?? "";
  const hostname = host.split(":")[0];

  if (hostname === SITE_HOST || !hostname.endsWith(`.${SITE_HOST}`)) {
    return NextResponse.next();
  }

  const subdomain = hostname.slice(0, -(SITE_HOST.length + 1));
  // Un'etichetta con un punto (es. un sottodominio "annidato" inatteso) o
  // riservata non va mai interpretata come slug di un blog.
  if (!subdomain || subdomain.includes(".") || RESERVED_SUBDOMAINS.has(subdomain)) {
    return NextResponse.next();
  }

  const firstSegment = request.nextUrl.pathname.split("/")[1];
  if (PLATFORM_ONLY_PATHS.has(firstSegment)) {
    return NextResponse.next();
  }

  const url = request.nextUrl.clone();
  url.pathname = `/${subdomain}${request.nextUrl.pathname}`;
  return NextResponse.rewrite(url);
}

export const config = {
  matcher: ["/((?!_next/|api/|favicon.ico|robots.txt|sitemap.xml|theme-init.js).*)"],
};
