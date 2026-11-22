*Ultimo aggiornamento: {{last_update}}*

Questa pagina spiega quali cookie e tecnologie simili usa {{instance_fqdn}} («il Servizio»), ai sensi dell'art. 5, paragrafo 3, della direttiva 2002/58/CE, dell'art. 122 del Codice in materia di protezione dei dati personali (D.Lgs. 196/2003) e delle Linee guida del Garante per la protezione dei dati personali del 10 giugno 2021. Per gli altri dati personali trattati vedi l'[Informativa sulla privacy](/p/privacy). Il Titolare del trattamento è **{{controller_name}}** ({{controller_email}}).

## In breve

Il Servizio usa **soltanto cookie tecnici**, necessari al suo funzionamento. Non usa cookie di profilazione, pubblicità o analisi del traffico, né strumenti che ti seguano da un sito all'altro. Per i cookie tecnici la legge non richiede il tuo consenso, perciò non vedrai un banner: resta l'obbligo di informarti, che questa pagina assolve.

## Cookie utilizzati

| Nome | A cosa serve | Durata | Tipo |
|---|---|---|---|
| `noct_refresh_token` | Mantiene la sessione dopo l'accesso, così non devi reinserire le credenziali a ogni pagina. Non è leggibile dagli script della pagina. | fino a {{session_days}} giorni, o fino all'uscita | Tecnico, prima parte |
| `noct_csrf_token` | Protegge le richieste che modificano dati da richieste false inviate da altri siti. | fino a {{session_days}} giorni, o fino all'uscita | Tecnico, prima parte |
{{#if sso}}
| `noct_sso_state` | Lega il tuo browser alla richiesta di accesso con un account esterno, per impedirne l'uso da parte di terzi. | 10 minuti | Tecnico, prima parte |
{{/if}}
| `notturni_ui_locale` | Ricorda la lingua dell'interfaccia che hai scelto. | 1 anno | Tecnico (preferenza), prima parte |

I cookie di accesso (`noct_refresh_token`, `noct_csrf_token`) vengono impostati solo se accedi; quello della lingua solo se scegli una lingua.

## Archiviazione locale del browser

Oltre ai cookie usiamo l'archiviazione locale del browser, che resta sul tuo dispositivo e non viene inviata al Servizio:

- `notturni_theme_mode`: ricorda se preferisci il tema chiaro, scuro o automatico;
- `notturni_dashboard_sidebar_collapsed`: ricorda se hai chiuso il menu laterale della tua area personale.

## Che cosa non facciamo

- Nessuno strumento di analisi del traffico, nessuna pubblicità, nessun pixel o impronta digitale del dispositivo.
- I caratteri tipografici sono serviti dal Servizio stesso, non da fornitori esterni.
- Le letture dei post sono contate come numero per giorno, senza cookie e senza identificare il lettore.

## Servizi di terze parti
{{#if turnstile}}

**Cloudflare Turnstile.** Nelle pagine dei post i cui commenti sono aperti anche a chi non ha un account, il tuo browser carica da Cloudflare un controllo anti-abuso che serve a distinguere le persone dagli automatismi. Può usare tecnologie di archiviazione sul dispositivo per funzionare ed è fornito da Cloudflare, che ne è responsabile; consulta la sua informativa per i dettagli. Non viene caricato altrove nel Servizio.
{{/if}}
{{#if sso}}

**Accesso con un account esterno.** Se scegli di accedere con un fornitore esterno vieni portato sul suo sito, che può usare i propri cookie secondo la propria informativa.
{{/if}}

**Contenuti incorporati e link.** Gli autori possono inserire immagini, link o contenuti ospitati altrove. Quando il tuo browser li carica, il sito che li ospita può ricevere dati tecnici (ad esempio il tuo indirizzo IP) e impostare i propri cookie: il Servizio non ne ha il controllo.

## Come gestire o cancellare i cookie

Puoi vedere, bloccare o cancellare i cookie dalle impostazioni del tuo browser. Se blocchi i cookie tecnici non potrai accedere all'area personale né mantenere la sessione; la navigazione dei contenuti pubblici continua a funzionare. Cancellando i cookie di accesso esci dal Servizio.

## Modifiche e contatti

Se cambiano i cookie usati aggiorneremo questa pagina e, se servisse, ti chiederemo il consenso. Per domande scrivi a {{controller_email}}. Per reclami puoi rivolgerti al Garante per la protezione dei dati personali ([www.garanteprivacy.it](https://www.garanteprivacy.it)).
