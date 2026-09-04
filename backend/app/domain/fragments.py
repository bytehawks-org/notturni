"""Frammenti di testo evidenziati dai lettori sui post (raccolta unificata,
vedi app/api/v1/fragments.py e frontend `/dashboard/frammenti`).

Il vincolo "massimo 15% del testo del post" è applicato qui contro la
lunghezza del Markdown grezzo salvato (`Post.content`), non del testo reso —
il backend non fa mai rendering Markdown (CLAUDE.md #1/#5: responsabilità del
frontend). È quindi un proxy, non lo stesso conteggio usato lato client sul
testo effettivamente visibile in pagina, ma con lo stesso ordine di
grandezza: sufficiente come vincolo di buon senso lato server, la selezione
effettiva la applica già il frontend sul testo visualizzato."""

MAX_FRAGMENT_RATIO = 0.15
MIN_FRAGMENT_LENGTH = 1


def validate_fragment_text(text: str, post_content: str) -> str:
    """Ripulisce e valida il testo di un frammento. Solleva ValueError se non valido."""
    cleaned = " ".join(text.split())
    if len(cleaned) < MIN_FRAGMENT_LENGTH:
        raise ValueError("Il frammento non può essere vuoto.")
    max_len = max(MIN_FRAGMENT_LENGTH, int(len(post_content) * MAX_FRAGMENT_RATIO))
    if len(cleaned) > max_len:
        raise ValueError("Il frammento selezionato supera il 15% del testo del post.")
    return cleaned
