import re

# Riservati per evitare ambiguità con segmenti di path fissi (es. /users/me)
# o con termini di sistema. Stesso principio della blacklist per gli slug dei
# blog, vedi app/domain/blog_rules.py.
RESERVED_USERNAMES = {
    "me",
    "admin",
    "api",
    "www",
    "root",
    "system",
    "notturni",
    "support",
    "help",
    "moderator",
    "moderatore",
    "null",
    "undefined",
}

USERNAME_MIN_LENGTH = 3
USERNAME_MAX_LENGTH = 32

# todo/USERS.md #1: lo username è un identificatore univoco e citabile come
# `@username` nei contenuti (vedi app/api/v1/blogs.py::mentionable_users e il
# rendering lato frontend). Il formato deve quindi essere prevedibile: solo
# minuscole/cifre, con `-`/`_` come separatori interni (mai a inizio/fine, mai
# doppi). Stessa filosofia dello slug dei blog.
_USERNAME_PATTERN = re.compile(r"^[a-z0-9]+(?:[-_][a-z0-9]+)*$")


def normalize_username_candidate(raw: str) -> str:
    """Rende una stringa arbitraria (es. la parte locale di un'email in SSO)
    conforme al formato username: minuscole/cifre con `-`/`_` interni, lunghezza
    minima garantita con padding. Non garantisce l'unicità (spetta a chi chiama)."""
    cleaned = re.sub(r"[^a-z0-9_-]", "", raw.lower())
    cleaned = re.sub(r"[-_]{2,}", "_", cleaned).strip("-_")
    if len(cleaned) < USERNAME_MIN_LENGTH:
        cleaned = (cleaned + "utente")[:USERNAME_MAX_LENGTH]
    return cleaned[:USERNAME_MAX_LENGTH].rstrip("-_")


def validate_username(username: str) -> None:
    if not (USERNAME_MIN_LENGTH <= len(username) <= USERNAME_MAX_LENGTH):
        raise ValueError(
            f"Lo username deve avere da {USERNAME_MIN_LENGTH} a {USERNAME_MAX_LENGTH} caratteri."
        )
    if not _USERNAME_PATTERN.fullmatch(username):
        raise ValueError(
            "Lo username può contenere solo lettere minuscole, cifre, '-' e '_' "
            "(non a inizio/fine e non ripetuti)."
        )
    if username.lower() in RESERVED_USERNAMES:
        raise ValueError(f"'{username}' è un nome utente riservato.")
