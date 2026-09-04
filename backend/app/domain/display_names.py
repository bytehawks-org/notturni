"""Nome pubblico "personale" di un utente, in base alla preferenza di profilo
`User.post_author_name_style` (dashboard/profilo): nome e cognome, alias
globale del profilo, o username (default).

Condiviso tra la risoluzione del nome autore di post/pagine
(app/api/v1/posts.py::_resolve_author_display_name, che sopra a questo
applica anche l'alias di membership/blog) e quella dei commenti
(app/api/v1/comments.py, dove l'alias di blog non si applica: si commenta
come sé stessi, non come il blog). Ricalcolato ad ogni lettura, non salvato
come snapshot: un cambio di username o di preferenza si riflette ovunque
senza dover risalvare nulla (CLAUDE.md #1, todo/USERS.md #2)."""

from app.models.user import PostAuthorNameStyle, User


def resolve_personal_display_name(user: User) -> str:
    if user.post_author_name_style == PostAuthorNameStyle.FULL_NAME:
        full = " ".join(part for part in (user.first_name, user.last_name) if part).strip()
        return full or user.username
    if user.post_author_name_style == PostAuthorNameStyle.DISPLAY_NAME:
        return user.display_name or user.username
    return user.username
