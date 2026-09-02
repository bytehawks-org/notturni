# Editor

> Stato: **@menzioni** e **note a piè di pagina con bibliografia del blog**
> sono implementate. `@username` diventa un link al profilo (autocomplete
> nell'editor, interruttore per blog). Le note sono un elenco strutturato del
> post: nel corpo il riferimento è il marcatore `[n](#nota-n)` inserito dal
> pulsante «Nota»; la pagina pubblica del post mostra l'elenco numerato in
> fondo + il testo come tooltip, e `/{blog}/bibliografia` raccoglie tutte le
> note dei post pubblicati (deduplicate, con i post che le citano). Restano da
> fare i riferimenti `[[...]]` e il menu comandi `/`.

- [DONE] Note (a piè di pagina + tooltip al passaggio del mouse) con bibliografia automatica globale per il blog.

Gestire un sistema di menzioni:

- [DONE] @ menzione si un utente del proprio blog (autore o lettore iscritto) come suggerimento, in alternativa scrivere il nome utente che si desidera menzionare
- [ ] [[]] riferimento ad una articolo del proprio blog
- [ ] [[{nome_blog}:]] riferimento ad un articolo di un altro blog
- [ ] prevedere una sorta di help in linea dei possibili comandi con digitando il carattere / (se digitato dopo uno spazio vuoto o una nuova riga)

I riferimenti si dovrebbe trasformare automaticamente in link e sarebbe perfetto se andassero a creare assieme ai tag una sorta di brain map dei concetti e dei temi trattati nel blog.

Le funzionalità di riferimento possono essere nel caso disabilitate dall'utente (attive di default). — fatto per le @menzioni (`Blog.mentions_enabled`), da estendere agli altri tipi di riferimento quando verranno implementati.
