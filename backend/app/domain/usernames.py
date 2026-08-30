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


def validate_username(username: str) -> None:
    if username.lower() in RESERVED_USERNAMES:
        raise ValueError(f"'{username}' è un nome utente riservato.")
