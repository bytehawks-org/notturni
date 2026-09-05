# Avvio locale — guida al primo test

Percorso più rapido per portare su tutto lo stack in locale e fare un primo
giro di prova, dalla registrazione alla pubblicazione di un post. Per il
dettaglio di ogni pezzo vedi [README.md](README.md) (panoramica e struttura),
[backend/API.md](backend/API.md) (riferimento endpoint) e i README di
[backend](backend/README.md)/[frontend](frontend/README.md).

## Prerequisiti

- [Podman](https://podman.io/) (rootless va bene).
- Un provider compose per Podman: `podman compose` nativo non è garantito su
  ogni installazione — se `podman compose version` fallisce, installa
  `podman-compose`.

  ```bash
  sudo dnf install podman-compose   # Fedora (ambiente di sviluppo di riferimento)
  # in alternativa, se il python di sistema non ha il modulo pip:
  python3 -m venv /tmp/podman-compose-venv && /tmp/podman-compose-venv/bin/pip install podman-compose
  # poi usa /tmp/podman-compose-venv/bin/podman-compose al posto di "podman compose"
  ```

Non serve altro installato a mano: Postgres, Redis, RabbitMQ, MinIO, backend
e frontend girano tutti in container.

## 1. Configurazione

```bash
cp .env.example .env
```

I valori di default in `.env.example` funzionano così come sono per un primo
test in locale — non serve modificare nulla. Due variabili valgono la pena di
essere notate:

- `NOCT_DEPLOYMENT_MODE=platform` — stack multiutente (comportamento di
  default). Con `solo` invece la registrazione si
  chiude dopo il primo utente, che diventa automaticamente Super Admin: utile
  per provare rapidamente il caso d'uso "blog personale" invece che quello
  multiutente. Per questa guida si assume `platform`.
- `NOCT_JWT_SECRET` / `NOCT_SESSION_SECRET` — vanno bene i default (`foo`) per
  un test locale; **non riusarli mai in un ambiente esposto** (vedi i commenti
  nel file per generarne di veri).
- `NOCT_SUPER_ADMIN_USERNAME`/`_EMAIL`/`_PASSWORD` — se tutte e tre
  valorizzate (i default in `.env.example` lo sono già), il backend crea
  questo account come Super Admin al primo avvio: nessun passaggio manuale
  per entrare nell'area di amministrazione (punto 4).

## 2. Avvio dello stack

```bash
podman compose up -d --build
# oppure, se non hai podman compose nativo:
podman-compose up -d --build
```

La prima volta impiega qualche minuto (build delle immagini + download di
Postgres/Redis/RabbitMQ/MinIO). Al termine dovresti avere 9 container su:

- Frontend (dashboard e amministrazione incluse): <http://localhost:3000>
- Backend — Swagger UI: <http://localhost:8000/docs>
- Backend — health check: <http://localhost:8000/api/v1/health>
- RabbitMQ — console: <http://localhost:15672> (admin/foo)
- MinIO — console: <http://localhost:9001> (admin/foobarfoobar)

Verifica rapida che il backend risponda e veda il database:

```bash
curl http://localhost:8000/api/v1/health
# {"status":"ok"}
```

## 3. Migrazioni del database

Non partono da sole all'avvio del container — vanno applicate una volta:

```bash
podman exec -it $(podman ps -qf "name=backend") alembic upgrade head
```

(Se hai usato `podman-compose`, il nome del container potrebbe avere un
suffisso diverso: `podman ps` per controllare, o cerca quello con l'immagine
`notturni_backend`/`notturni-backend`.)

## 4. Primo utente

Con i default di `.env.example` non serve fare nulla: il backend crea da solo
un Super Admin al primo avvio, da `NOCT_SUPER_ADMIN_USERNAME`/`_EMAIL`/
`_PASSWORD` (idempotente — un riavvio successivo non lo tocca). Accedi
direttamente su <http://localhost:3000> con quelle credenziali: le voci di
amministrazione compaiono nel menu del dashboard.

Se preferisci un tuo account invece del preset (o hai svuotato quelle
variabili), registra un utente normale dall'interfaccia (punto 5) o via API,
poi promuovilo a mano:

```bash
podman exec -it $(podman ps -qf "name=postgres") \
  psql -U admin -d notturni -c \
  "UPDATE users SET platform_role='super_admin' WHERE username='<tuo-username>';"
```

Rifai login dopo la promozione (il ruolo viene letto al login, non aggiornato
in una sessione già aperta). Se invece hai impostato `NOCT_DEPLOYMENT_MODE=solo`
nel `.env`, questo passaggio non serve: il primo utente registrato è già
Super Admin.

Per l'accesso diretto all'API senza passare da editor/admin (motore core,
script), vedi lo script di bootstrap in [backend/API.md](backend/API.md#come-ottenere-il-primo-token).

## 5. Primo giro di test — interfaccia web

1. Apri <http://localhost:3000>, clicca **Registrati** e crea un account.
2. Vieni portato alla dashboard: clicca **Nuovo blog**, scegli uno slug (almeno
   4 caratteri) e un titolo.
3. Entra nel blog appena creato → tab **Post** → **Nuovo post**: scrivi un
   titolo, uno slug e un contenuto (Markdown) e crea la bozza.
4. Nell'editor del post clicca **Pubblica**.
5. Torna alla lista: il post ora è pubblico. Verificalo anche via API con
   `curl http://localhost:8000/api/v1/blogs/<slug-del-blog>/posts`.
6. Prova la tab **Aspetto** per cambiare la palette del blog, e **Impostazioni**
   per aprire i commenti anche a chi non è registrato.
7. Vai su **Profilo** (menu in alto): imposta una bio, carica un avatar,
   aggiungi un link social. Se l'utente con cui hai fatto login è Super
   Admin/Amministratore (con i default del punto 4 è un account separato da
   quello appena registrato: rifai login con le credenziali
   `NOCT_SUPER_ADMIN_*`, o con l'utente promosso a mano se hai seguito
   l'alternativa del punto 4), compaiono nello stesso menu anche **Pagine**
   (crea "Chi siamo", "Privacy", ecc., stesso editor dei post), **Utenti**
   (ruoli/attivazione — solo in modalità `platform`), **Tutti i blog**
   (elenco di tutti i blog della piattaforma, con ricerca e sospensione) e
   **Moderazione** (elenco di tutti i post della piattaforma, con ricerca e
   nascondi/mostra il singolo post). Ogni sezione ha un campo di ricerca.
8. Prova il selettore del tema (chiaro/scuro/automatico) in alto a destra —
   in automatico il browser chiederà il permesso di geolocalizzazione per
   calcolare alba/tramonto; se lo neghi, ripiega sulle preferenze di sistema.

## 6. Primo giro di test — API diretta

Alternativa (o complemento) al punto 5, utile per capire cosa sta succedendo
sotto al frontend. Riferimento completo in [backend/API.md](backend/API.md).

```bash
# registrazione + login
curl -s -X POST http://localhost:8000/api/v1/auth/register \
  -H "Content-Type: application/json" \
  -d '{"username":"prova","email":"prova@example.com","password":"Password123!"}'

TOKEN=$(curl -s -X POST http://localhost:8000/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"prova@example.com","password":"Password123!"}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['access_token'])")

# crea un blog
curl -s -X POST http://localhost:8000/api/v1/blogs \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"slug":"blog-di-prova","title":"Blog di prova"}'

# crea e pubblica un post
POST_ID=$(curl -s -X POST http://localhost:8000/api/v1/blogs/blog-di-prova/posts \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"slug":"primo-post","title":"Ciao mondo","content":"# Funziona!"}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['id'])")

curl -s -X POST http://localhost:8000/api/v1/posts/$POST_ID/publish \
  -H "Authorization: Bearer $TOKEN"

# verifica pubblica
curl -s http://localhost:8000/api/v1/blogs/blog-di-prova/posts
```

## 7. Fermare / ripulire l'ambiente

```bash
podman compose down          # ferma i container, mantiene i dati (volumi)
podman compose down -v       # ferma e cancella anche i dati (riparti da zero)
```

## 8. Problemi comuni

- **`podman compose` non trovato / "looking up compose provider failed"** —
  installa `podman-compose` (vedi Prerequisiti) e usa `podman-compose` al
  posto di `podman compose` in tutti i comandi sopra.
- **Il container `rabbitmq` si riavvia una volta all'avvio** — capita su
  Podman rootless, un noto problema di permessi al primissimo boot di questa
  immagine (`.erlang.cookie`); si auto-risolve al riavvio (`restart:
  on-failure` in `compose.yaml`). Se dopo un paio di riavvii resta giù,
  guardane i log con `podman logs $(podman ps -aqf "name=rabbitmq")`.
- **Il backend risponde ma ogni chiamata autenticata dà errore** — hai
  applicato le migrazioni (punto 3)? Senza schema, molte tabelle non esistono.
- **Il frontend non riesce a chiamare il backend (errori di rete/CORS in
  console)** — succede se il frontend gira su un URL diverso da
  `http://localhost:3000`: `NOCT_CORS_ORIGINS` nel `.env` deve includerlo.
- **Upload avatar/media/aspetto non funziona** — con il backend di storage
  di default (`NOCT_STORAGE_BACKEND=s3`) verifica che il container `minio`
  sia `Up` (`podman ps`); i bucket si creano automaticamente al primo upload.
  In alternativa esiste `NOCT_STORAGE_BACKEND=localstorage` (filesystem
  locale servito dal backend stesso, senza bisogno di MinIO/S3 — vedi
  `.env.example`), non usato di default in questo stack.
- **Il codice MFA via email non arriva mai** — atteso: nessun provider email
  reale è configurato in questo progetto, il consumer (`worker-email-otp` nel
  `compose.yaml`) logga soltanto il codice invece di spedirlo — guardane i log
  con `podman logs -f $(podman ps -qf "name=worker-email-otp")`.
- **Voglio essere sicuro che il backup dei post su S3 funzioni** — controlla i
  log del worker dedicato: `podman logs -f $(podman ps -qf "name=worker-post-backup")`.

## Prossimi passi

Una volta verificato che tutto gira, per approfondire:

- [README.md](README.md) — panoramica, stack, struttura del repository.
- [backend/API.md](backend/API.md) — ogni endpoint, con esempi di
  richiesta/risposta e le regole di autorizzazione.
- [ROADMAP.md](ROADMAP.md) — tabella completa delle specifiche di prodotto,
  il loro stato di avanzamento e il backlog dei prossimi sviluppi.
- [k8s/README.md](k8s/README.md) — manifest per un deploy su Kubernetes (bozza iniziale).
