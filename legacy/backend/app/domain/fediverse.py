"""Identificativi per una futura federazione (AT Protocol/Bluesky, poi
ActivityPub/Mastodon — ROADMAP.md §5, priorità in quest'ordine): creati come
placeholder, non persistiti, non federati realmente in questo blocco — solo
calcolati e mostrati nella schermata di profilo (CLAUDE.md #5).

Derivati da `user.id` (UUID stabile, mai da `user.username`): un identificativo
federato deve restare stabile anche se lo username cambia, altrimenti
romperebbe la sua stessa ragion d'essere non appena l'utente rinominasse
l'account."""

from app.core.config import settings
from app.models.user import User


def atproto_did_for(user: User) -> str:
    """`did:web` invece di `did:plc`: nessuna registrazione esterna
    richiesta, risolvibile in futuro pubblicando un documento JSON su un URL
    sotto il nostro controllo."""
    return f"did:web:{settings.instance_fqdn}:users:{user.id}"


def activitypub_actor_id_for(user: User) -> str:
    """URL dell'attore ActivityPub, convenzione standard — nessun endpoint
    `/ap/actors/...` reale servito oggi, nessun WebFinger: solo la stringa
    identificativa."""
    return f"https://{settings.instance_fqdn}/ap/actors/{user.id}"
