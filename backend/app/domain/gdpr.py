"""Export e cancellazione self-service dei dati dell'account (ROADMAP.md §1,
GDPR — diritto di accesso/portabilità Art. 20 e di cancellazione Art. 17).

**Cancellazione = anonimizzazione, non `DELETE` della riga `users`.** Un
utente può essere proprietario di blog pubblici con altri collaboratori, o
aver scritto post/commenti su blog altrui: cancellare fisicamente la riga
richiederebbe o bloccare l'operazione finché non esistono più blog/post/
commenti collegati (nessuna funzionalità di trasferimento/cancellazione blog
esiste oggi), oppure cancellare a cascata anche quel contenuto — danneggiando
altri collaboratori e lettori che non hanno chiesto nulla. L'anonimizzazione
è la via prevista anche dal GDPR quando l'erasure in senso stretto confligge
con diritti di terzi (Art. 17.3): i dati personali vengono cancellati, il
contenuto pubblico condiviso resta, attribuito a un'identità anonima (stesso
principio già presente nel prodotto con gli alias per-blog, CLAUDE.md #1).

Cancellati per intero (dati puramente personali, mai condivisi con altri):
sessioni, token API, codici MFA email pendenti, identità SSO, link social,
frammenti salvati, i follow (in entrambe le direzioni) e le membership su
blog altrui (l'utente anonimizzato non ha più senso come "collaboratore
attivo"). Lasciati intatti: blog di proprietà, post, commenti — il
`post_author_name_style`/`display_name` impostati qui li fa comparire da
subito con l'autore "Utente eliminato" ovunque (stessa risoluzione dinamica
già usata per gli alias, `app/domain/display_names.py`)."""

from typing import Any

from sqlalchemy import delete, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.core.storage import avatar_public_url, delete_avatar
from app.models.api_token import ApiToken
from app.models.audit_log import AuditLog
from app.models.blog import Blog, BlogMembership
from app.models.comment import Comment
from app.models.follow import BlogFollow, UserFollow
from app.models.mfa_email_code import MfaEmailCode
from app.models.post import Post
from app.models.post_fragment import PostFragment
from app.models.social_link import SocialLink
from app.models.sso_identity import SsoIdentity
from app.models.user import PlatformRole, PostAuthorNameStyle, User
from app.models.user_session import UserSession

ANONYMIZED_DISPLAY_NAME = "Utente eliminato"
# limite difensivo sull'estratto del registro di audit personale: un account
# molto vecchio/attivo non deve poter generare un export senza limiti
MAX_EXPORTED_AUDIT_EVENTS = 1000


async def export_user_data(session: AsyncSession, user: User) -> dict[str, Any]:
    """Istantanea dei dati collegati all'utente (Art. 20 GDPR — portabilità).
    Include il contenuto scritto (post/commenti) ovunque si trovi, non solo
    sui propri blog: è comunque testo scritto dall'utente stesso."""
    social_links = (
        (await session.execute(select(SocialLink).where(SocialLink.user_id == user.id)))
        .scalars()
        .all()
    )
    blogs_owned = (await session.execute(select(Blog).where(Blog.owner_id == user.id))).scalars().all()
    posts_authored = (
        (await session.execute(select(Post).where(Post.author_id == user.id))).scalars().all()
    )
    comments_authored = (
        (await session.execute(select(Comment).where(Comment.author_id == user.id))).scalars().all()
    )
    fragments = (
        (await session.execute(select(PostFragment).where(PostFragment.user_id == user.id)))
        .scalars()
        .all()
    )
    following = (
        (await session.execute(select(UserFollow).where(UserFollow.follower_id == user.id)))
        .scalars()
        .all()
    )
    followers = (
        (await session.execute(select(UserFollow).where(UserFollow.followed_user_id == user.id)))
        .scalars()
        .all()
    )
    blogs_followed = (
        (await session.execute(select(BlogFollow).where(BlogFollow.follower_id == user.id)))
        .scalars()
        .all()
    )
    api_tokens = (
        (await session.execute(select(ApiToken).where(ApiToken.user_id == user.id))).scalars().all()
    )
    audit_events = (
        (
            await session.execute(
                select(AuditLog)
                .where(AuditLog.actor_id == user.id)
                .order_by(AuditLog.occurred_at.desc())
                .limit(MAX_EXPORTED_AUDIT_EVENTS)
            )
        )
        .scalars()
        .all()
    )

    return {
        "account": {
            "id": str(user.id),
            "username": user.username,
            "email": user.email,
            "display_name": user.display_name,
            "first_name": user.first_name,
            "last_name": user.last_name,
            "bio": user.bio,
            "country": user.country,
            "native_language": user.native_language,
            "fallback_languages": user.fallback_languages,
            "platform_role": user.platform_role.value,
            "mfa_enabled": user.mfa_enabled,
            "created_at": user.created_at.isoformat(),
        },
        "social_links": [
            {"label": s.label, "url": s.url, "position": s.position} for s in social_links
        ],
        "blogs_owned": [
            {
                "slug": b.slug,
                "title": b.title,
                "subtitle": b.subtitle,
                "description": b.description,
                "visibility": b.visibility.value,
                "created_at": b.created_at.isoformat(),
            }
            for b in blogs_owned
        ],
        "posts_authored": [
            {
                "id": str(p.id),
                "blog_id": str(p.blog_id),
                "title": p.title,
                "slug": p.slug,
                "locale": p.locale,
                "status": p.status.value,
                "content": p.content,
                "created_at": p.created_at.isoformat(),
                "published_at": p.published_at.isoformat() if p.published_at else None,
            }
            for p in posts_authored
        ],
        "comments_authored": [
            {
                "id": str(c.id),
                "post_id": str(c.post_id),
                "content": c.content,
                "status": c.status.value,
                "created_at": c.created_at.isoformat(),
            }
            for c in comments_authored
        ],
        "fragments": [
            {"post_id": str(f.post_id), "text": f.text, "captured_at": f.created_at.isoformat()}
            for f in fragments
        ],
        "follows": {
            "following_user_ids": [str(f.followed_user_id) for f in following],
            "follower_user_ids": [str(f.follower_id) for f in followers],
            "blogs_followed_ids": [str(f.blog_id) for f in blogs_followed],
        },
        "api_tokens": [
            {
                "name": t.name,
                "token_prefix": t.token_prefix,
                "created_at": t.created_at.isoformat(),
                "last_used_at": t.last_used_at.isoformat() if t.last_used_at else None,
                "revoked_at": t.revoked_at.isoformat() if t.revoked_at else None,
            }
            for t in api_tokens
        ],
        "audit_log": [
            {
                "occurred_at": a.occurred_at.isoformat(),
                "action": a.action,
                "ip": str(a.ip) if a.ip is not None else None,
                "user_agent": a.user_agent,
            }
            for a in audit_events
        ],
    }


async def anonymize_and_deactivate_user(session: AsyncSession, user: User) -> None:
    """Cancella i dati puramente personali e anonimizza l'account (vedi
    docstring del modulo per cosa resta e perché). Non fa il `commit`: il
    chiamante lo fa nella stessa transazione dell'evento di audit."""
    if user.avatar_object_key is not None:
        delete_avatar(user.avatar_object_key)

    await session.execute(delete(UserSession).where(UserSession.user_id == user.id))
    await session.execute(delete(ApiToken).where(ApiToken.user_id == user.id))
    await session.execute(delete(MfaEmailCode).where(MfaEmailCode.user_id == user.id))
    await session.execute(delete(SsoIdentity).where(SsoIdentity.user_id == user.id))
    await session.execute(delete(SocialLink).where(SocialLink.user_id == user.id))
    await session.execute(delete(PostFragment).where(PostFragment.user_id == user.id))
    await session.execute(
        delete(UserFollow).where(
            (UserFollow.follower_id == user.id) | (UserFollow.followed_user_id == user.id)
        )
    )
    await session.execute(delete(BlogFollow).where(BlogFollow.follower_id == user.id))
    # membership come collaboratore su blog altrui — sui propri blog non ne
    # ha bisogno (il possesso è Blog.owner_id, non una membership)
    await session.execute(delete(BlogMembership).where(BlogMembership.user_id == user.id))

    anon_suffix = user.id.hex[:12]
    user.username = f"utente-eliminato-{anon_suffix}"
    user.email = f"deleted-{user.id}@notturni.invalid"
    user.hashed_password = None
    user.platform_role = PlatformRole.UTENTE
    user.is_active = False
    user.mfa_enabled = False
    user.mfa_method = None
    user.mfa_totp_secret = None
    user.bio = None
    user.first_name = None
    user.last_name = None
    user.display_name = ANONYMIZED_DISPLAY_NAME
    user.post_author_name_style = PostAuthorNameStyle.DISPLAY_NAME
    user.country = None
    user.native_language = None
    user.fallback_languages = []
    user.avatar_object_key = None
