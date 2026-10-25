# syntax=docker/dockerfile:1

FROM node:22-slim AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-slim AS builder
WORKDIR /app
# URL del sito, dell'API e nome della piattaforma si risolvono a RUNTIME dalle env del container
# (NOCT_SITE_URL / NOCT_INSTANCE_FQDN, NOCT_API_PUBLIC_URL, NOCT_SITE_NAME — vedi src/lib/site.ts):
# la stessa immagine serve qualunque dominio. I build-arg NEXT_PUBLIC_* qui sotto sono solo un
# fallback per lo sviluppo locale (inglobato nel bundle); vuoti di default = stessa origine.
ARG NEXT_PUBLIC_API_URL=
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
# URL pubblico del sito (SEO: metadataBase/canonical/sitemap.xml/robots.txt —
# vedi frontend/src/lib/site.ts), stesso schema di NEXT_PUBLIC_API_URL sopra.
ARG NEXT_PUBLIC_SITE_URL=
ENV NEXT_PUBLIC_SITE_URL=$NEXT_PUBLIC_SITE_URL
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npm run build

FROM node:22-slim AS runner
WORKDIR /app
ENV NODE_ENV=production

RUN groupadd --system nodejs && useradd --system --gid nodejs nextjs

COPY --from=builder /app/public ./public
COPY --from=builder --chown=nextjs:nodejs /app/.next/standalone ./
COPY --from=builder --chown=nextjs:nodejs /app/.next/static ./.next/static

USER nextjs
EXPOSE 3000

CMD ["node", "server.js"]
