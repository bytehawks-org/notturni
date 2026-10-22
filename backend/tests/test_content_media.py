"""Estrazione di media/link dal Markdown (app/domain/content_media.py):
correttezza dell'URL con parentesi bilanciate (es. Wikipedia) e assenza di
backtracking polinomiale su input patologici (CodeQL py/polynomial-redos)."""

import time

from app.domain.content_media import extract_links, extract_media


def test_extract_media_basic() -> None:
    content = '![alt text](https://example.com/img.png "sensitive:nudity")'
    refs = extract_media(content)
    assert len(refs) == 1
    assert refs[0].url == "https://example.com/img.png"
    assert refs[0].alt_text == "alt text"
    assert refs[0].categories == ("nudity",)
    assert refs[0].is_sensitive is True


def test_extract_media_flagged_by_automod_without_category() -> None:
    """`categories` vuoto non implica "non sensibile": l'automoderazione (o
    il modal senza una categoria specifica scelta) marca solo `"sensitive"`,
    senza `:categoria` — `is_sensitive` è il campo che lo distingue da
    un'immagine non segnalata affatto (bug corretto a valle, vedi
    backend/app/models/post_media.py e la bibliografia media pubblica)."""
    content = '![alt](https://example.com/img.png "sensitive")'
    refs = extract_media(content)
    assert len(refs) == 1
    assert refs[0].categories == ()
    assert refs[0].is_sensitive is True


def test_extract_media_not_flagged() -> None:
    content = "![alt](https://example.com/img.png)"
    refs = extract_media(content)
    assert len(refs) == 1
    assert refs[0].categories == ()
    assert refs[0].is_sensitive is False


def test_extract_media_url_with_balanced_parentheses() -> None:
    """Un URL non escapato con una coppia di parentesi bilanciate (comune su
    Wikipedia) va estratto per intero, non troncato alla prima "(" ."""
    content = "![alt](https://en.wikipedia.org/wiki/Example_(disambiguation))"
    refs = extract_media(content)
    assert len(refs) == 1
    assert refs[0].url == "https://en.wikipedia.org/wiki/Example_(disambiguation)"


def test_extract_links_excludes_footnote_anchors() -> None:
    content = "Testo [nota](#nota-1) e [link vero](https://example.com)."
    refs = extract_links(content)
    assert [r.url for r in refs] == ["https://example.com"]


def test_extract_media_is_not_polynomial_on_pathological_input() -> None:
    """Molte occorrenze di '![](' senza mai una ')' di chiusura facevano
    esplodere il vecchio `\\S+?` lazy (ri-scansione quasi fino a fine stringa
    a ogni posizione, O(n²)) — con la regex attuale resta lineare. Input
    volutamente più piccolo di quanto servirebbe a "sentire" un vero O(n²)
    (che qui esploderebbe comunque su qualsiasi runner) solo per tenere il
    test lineare-ma-lento sotto soglia anche su CI sotto carico, senza
    perdere la capacità di intercettare una regressione polinomiale."""
    payload = "![](" * 5_000
    start = time.monotonic()
    extract_media(payload)
    elapsed = time.monotonic() - start
    assert elapsed < 2.0, f"extract_media troppo lento su input patologico: {elapsed:.2f}s"
