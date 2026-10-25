*Ultimo aggiornamento: {{last_update}}*

Questa informativa spiega come {{instance_fqdn}} («il Servizio») tratta i dati personali di chi lo visita e di chi lo usa, ai sensi degli articoli 13 e 14 del Regolamento (UE) 2016/679 («GDPR»). Il Servizio è una piattaforma di pubblicazione di testi (blog, pagine e newsletter) in cui ogni utente decide cosa rendere pubblico.

## Titolare del trattamento

- **{{controller_name}}**
- Indirizzo: {{controller_address}}
- Contatto per la privacy: {{controller_email}}
{{#if controller_registry}}
- Dati di registrazione: {{controller_registry}}
{{/if}}
{{#if dpo_contact}}
- Responsabile della protezione dei dati (DPO): {{dpo_contact}}
{{/if}}
{{#unless dpo_contact}}
- Il Titolare non ha designato un Responsabile della protezione dei dati (DPO), non essendovi tenuto.
{{/unless}}

## Che cosa copre questa informativa

Copre i dati trattati dal Servizio. Non copre i siti raggiungibili tramite link e i contenuti incorporati dagli autori. I testi, le immagini e i commenti che un utente pubblica sono da lui scelti e sotto la sua responsabilità: se contengono dati di altre persone, quell'utente è tenuto a rispettare la normativa. Per segnalare un contenuto che viola i tuoi diritti scrivi al contatto indicato sopra{{#if reports}} oppure usa la funzione «Segnala» presente sui contenuti{{/if}}.

## Quali dati trattiamo, perché e su quale base

**Account e accesso.** Nome utente, indirizzo email, ruolo e lingua dell'interfaccia; la password è conservata soltanto come hash (Argon2id) e non è leggibile da nessuno. Servono a creare e gestire l'account e a farti accedere. Base giuridica: esecuzione del contratto (art. 6.1.b GDPR).

**Profilo e firma.** Nome visualizzato, nome e cognome, biografia, avatar, paese, lingua madre, interessi e dominio personale verificato sono tutti facoltativi. Il nome con cui firmi i tuoi testi può essere diverso dal tuo nome reale: puoi usare un alias, anche diverso per ogni blog. Ciò che inserisci nel profilo pubblico è visibile a chiunque; puoi uscire dall'elenco pubblico degli utenti dalle impostazioni del profilo. Base giuridica: esecuzione del contratto e, per i campi facoltativi, il tuo consenso (art. 6.1.a).

**Contenuti.** Post, pagine, note, frammenti, commenti, immagini e altri file che carichi. Li trattiamo per pubblicarli, mostrarli ai lettori che hai scelto (pubblico, solo iscritti o privato) e conservarli. Base giuridica: esecuzione del contratto.

**Sicurezza dell'account.** Sessioni di accesso (conserviamo solo l'impronta del token di sessione e la sua scadenza), token API (solo impronta e prefisso){{#if mfa}}, autenticazione a due fattori (il segreto TOTP è cifrato a riposo; i codici inviati per email sono temporanei){{/if}}. Base giuridica: esecuzione del contratto e legittimo interesse alla sicurezza (art. 6.1.f e art. 32 GDPR).

**Registro delle attività (log di audit).** Per sicurezza e responsabilità registriamo le azioni rilevanti (accessi, modifiche all'account, azioni di amministrazione e moderazione) con data, autore dell'azione (nome utente ed email al momento del fatto), indirizzo IP e user agent del browser. Base giuridica: legittimo interesse a proteggere il Servizio e a rendere conto di ciò che accade (art. 6.1.f).

**Protezione dagli abusi.** Per limitare tentativi ripetuti di accesso e richieste anomale teniamo temporaneamente in memoria l'indirizzo IP da cui arrivano le richieste. Base giuridica: legittimo interesse (art. 6.1.f).

**Comunicazioni di servizio.**{{#if smtp}} Ti scriviamo per conferme, codici di accesso, reimpostazione della password, inviti a collaborare e avvisi sul tuo account. Sono email necessarie al Servizio e non hanno finalità promozionali. Base giuridica: esecuzione del contratto.{{/if}}{{#unless smtp}} Questa istanza non è configurata per inviare email.{{/unless}}
{{#if sso}}

**Accesso con un account esterno.** Se scegli di accedere con{{#if sso_google}} Google{{/if}}{{#if sso_microsoft}} Microsoft{{/if}}{{#if sso_github}} GitHub{{/if}}{{#if sso_linkedin}} LinkedIn{{/if}}, ricevi dal fornitore e conserviamo il suo identificativo per il tuo account e l'indirizzo email associato. Il fornitore tratta i tuoi dati come titolare autonomo secondo la propria informativa. Base giuridica: esecuzione del contratto.
{{/if}}
{{#if newsletter}}

**Newsletter.** Se ti iscrivi a una newsletter trattiamo il tuo indirizzo email, la lingua, lo stato dell'iscrizione e, come prova del consenso, data, indirizzo IP e user agent della conferma. L'iscrizione richiede una email di conferma (doppio consenso) e ogni messaggio contiene un link per disiscriverti e per cancellare i tuoi dati senza accedere. Base giuridica: consenso (art. 6.1.a), revocabile in ogni momento.
{{/if}}
{{#if comments}}

**Commenti.** I commenti sono visibili secondo le impostazioni del blog. Chi è registrato commenta con il proprio nome visualizzato.{{#if turnstile}} Se l'autore del blog consente commenti anche a chi non è registrato, ti chiediamo il nome e l'email che vuoi indicare; questi commenti sono moderati prima della pubblicazione e il modulo è protetto da un controllo anti-abuso (vedi «Con chi condividiamo i dati»).{{/if}} Base giuridica: esecuzione del contratto per gli utenti registrati; legittimo interesse alla gestione e alla moderazione dei commenti per gli altri (art. 6.1.f).
{{/if}}
{{#if discovery}}

**Letture e seguiti.** Per mostrare agli autori quante volte è stata letta una pagina contiamo le letture come numero per post e per giorno, senza cookie e senza alcun identificativo del lettore. Se segui un blog o un utente conserviamo il collegamento tra i due account, che ti serve a ricevere il feed personale. Base giuridica: esecuzione del contratto e legittimo interesse a offrire statistiche aggregate.
{{/if}}
{{#if reports}}

**Segnalazioni e moderazione.** Se segnali un contenuto conserviamo la segnalazione, il suo autore (il segnalante) e le decisioni dei moderatori. Base giuridica: legittimo interesse a mantenere il Servizio sicuro e adempimento degli obblighi di legge sui contenuti illeciti (art. 6.1.c e 6.1.f).
{{/if}}
{{#if image_moderation}}

**Analisi automatica delle immagini.** Le immagini caricate sono esaminate da un software installato sui server del Servizio, senza inviarle a terzi, per riconoscere nudità e contenuti sensibili e mostrarle sfocate con un avviso. L'esito non produce effetti giuridici nei tuoi confronti e non costituisce profilazione; puoi contestarlo a un moderatore. Base giuridica: legittimo interesse alla tutela dei lettori (art. 6.1.f).
{{/if}}
{{#if custom_domains}}

**Dominio personale.** Se colleghi un tuo dominio verifichiamo che ti appartenga con una interrogazione DNS e conserviamo il dominio e l'esito. Base giuridica: esecuzione del contratto.
{{/if}}
{{#if git_sync}}

**Sincronizzazione con un repository Git.** Se la attivi, il Servizio invia i testi del tuo spazio al repository che indichi e conserva, cifrata, la credenziale (token o chiave di deploy) necessaria. Il repository è un servizio scelto da te e i dati inviati ne seguono le regole; cancellare un contenuto o l'account qui non lo cancella lì. Base giuridica: esecuzione del contratto su tua richiesta.
{{/if}}
{{#if link_preview}}

**Anteprime dei link.** Quando incolli un indirizzo nell'editor, il nostro server lo visita per leggere titolo e immagine e mostrarne l'anteprima: la richiesta parte dal server del Servizio, non dal tuo dispositivo. Base giuridica: esecuzione del contratto.
{{/if}}

**Dati di navigazione.** I server registrano tecnicamente gli accessi (indirizzo IP, data e ora, pagina richiesta, esito) per il funzionamento e la sicurezza; non li usiamo per profilarti. Il Servizio non usa strumenti di analisi del traffico né pubblicità. Per i cookie vedi la [Cookie policy](/p/cookie-policy).

Il conferimento dei dati dell'account è necessario per usare il Servizio; i campi facoltativi del profilo no. Senza i dati dell'account non possiamo offrirti l'accesso.

## Con chi condividiamo i dati

Non vendiamo i dati e non li usiamo per pubblicità o profilazione. Possono accedervi:

- il personale autorizzato del Titolare (amministratori e moderatori), solo per quanto serve al ruolo; gli accessi di amministrazione sono registrati;
- i fornitori che trattano i dati per nostro conto come responsabili del trattamento (art. 28 GDPR): {{hosting_description}}{{#if processors}}; {{processors}}{{/if}};
- le autorità, se la legge lo richiede.

**Altri soggetti coinvolti.**{{#if turnstile}} Nelle pagine dei post i cui commenti sono aperti anche a chi non è registrato, il tuo browser carica un controllo anti-abuso di Cloudflare (Turnstile), che riceve dati tecnici del dispositivo per distinguere persone e automatismi.{{/if}}{{#if sso}} Se accedi con un account esterno il relativo fornitore riceve i dati necessari all'accesso.{{/if}} I contenuti pubblici possono essere letti da chiunque, anche senza account, e indicizzati dai motori di ricerca secondo le impostazioni scelte dall'autore del blog.
{{#if transfers}}

## Trasferimenti fuori dallo Spazio economico europeo

Alcuni dei soggetti indicati possono trattare dati fuori dallo SEE. In tal caso il trasferimento avviene sulla base di una decisione di adeguatezza della Commissione europea o di clausole contrattuali standard, con le garanzie previste dagli artt. 44-49 GDPR. Per ottenere copia delle garanzie scrivi al contatto indicato in «Titolare del trattamento».
{{/if}}

## Per quanto tempo conserviamo i dati

- **Account e contenuti**: finché l'account è attivo. Quando lo cancelli l'account viene anonimizzato (vedi «I tuoi diritti»).
- **Sessioni di accesso**: scadono dopo {{session_days}} giorni di validità.
- **Registro delle attività**: {{audit_days}} giorni nel database.{{#if audit_archive}} Prima della cancellazione gli eventi vengono copiati in un archivio protetto, conservato {{audit_archive_retention}}.{{/if}}
- **Iscrizioni alla newsletter**: finché resti iscritto; puoi cancellare i tuoi dati in qualsiasi momento dal link in ogni email.
- **Blog eliminati**: restano recuperabili per {{grace_days}} giorni, poi vengono eliminati definitivamente.
{{#if backup}}
- **Copie di sicurezza**: {{backup_retention}}.
{{/if}}
- **Richieste GDPR**: gestite entro {{deletion_days}} giorni dalla ricezione.

## I tuoi diritti

Hai diritto di accedere ai tuoi dati, rettificarli, cancellarli, limitarne il trattamento, riceverli in formato strutturato e leggibile da dispositivo automatico (portabilità) e opporti al trattamento basato sul legittimo interesse (artt. 15-22 GDPR). Se il trattamento si basa sul consenso puoi revocarlo in ogni momento, senza pregiudicare i trattamenti già effettuati.

{{#if gdpr_tools}}
Dalla tua dashboard puoi **esportare tutti i tuoi dati** in formato JSON e **cancellare il tuo account**. Per altre richieste scrivi a {{controller_email}}.
{{/if}}
{{#unless gdpr_tools}}
Per esercitare i tuoi diritti scrivi a {{controller_email}}.
{{/unless}}
Rispondiamo entro un mese dalla richiesta (art. 12 GDPR), prorogabile in casi complessi.

**Che cosa significa «cancellare l'account».** L'account viene anonimizzato: nome utente ed email sono sostituiti da valori anonimi, le sessioni, i token, i seguiti e i collegamenti con provider esterni vengono eliminati e non è più possibile accedere. I blog, i post e i commenti che hai pubblicato restano visibili con l'autore «Utente eliminato», perché contengono anche il lavoro e i diritti di altre persone (art. 17.3 GDPR); se vuoi che un contenuto sia rimosso, eliminalo prima o chiedicelo. Il registro delle attività non viene cancellato prima della scadenza indicata in «Per quanto tempo conserviamo i dati», perché serve alla sicurezza e a rendere conto delle azioni svolte.

Hai inoltre diritto di proporre reclamo all'autorità di controllo: in Italia il Garante per la protezione dei dati personali ([www.garanteprivacy.it](https://www.garanteprivacy.it)).

## Sicurezza

Adottiamo misure tecniche e organizzative adeguate (art. 32 GDPR), tra cui connessioni cifrate, password conservate solo come hash, cifratura a riposo dei segreti, controllo degli accessi per ruolo, registro delle attività e limitazione dei tentativi di accesso.

## Minori

Il Servizio non è rivolto a chi ha meno di {{minimum_age|14}} anni e non raccogliamo consapevolmente dati di minori di tale età. Se ritieni che un minore ci abbia fornito dati, scrivici e li cancelleremo.

## Decisioni automatizzate

Non prendiamo decisioni basate unicamente su trattamenti automatizzati che producano effetti giuridici nei tuoi confronti (art. 22 GDPR).

## Modifiche

Possiamo aggiornare questa informativa, ad esempio quando cambiano le funzioni del Servizio. La data in alto indica l'ultima revisione; per le modifiche rilevanti ti avviseremo in modo adeguato.
