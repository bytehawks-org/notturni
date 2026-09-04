import uuid
from datetime import datetime, timezone
from typing import Any

from fastapi import APIRouter, Depends, HTTPException, UploadFile, status
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.ext.asyncio import AsyncSession
from sqlalchemy.orm import selectinload

from app.api.deps import get_current_user, get_optional_current_user
from app.core.database import get_session
from app.core.storage import content_public_url, upload_media
from app.domain.authorization import can_view_blog, can_write_posts, get_membership
from app.domain.blog_config import DEFAULT_BLOG_CONFIG, validate_blog_config
from app.domain.blog_rules import (
    assert_can_create_blog,
    validate_blog_description,
    validate_blog_slug,
    validate_blog_subtitle,
)
from app.api.v1.pages import (
    PageCreateRequest,
    PageOut,
    PageTranslationRequest,
    PageTranslationSummaryOut,
    PageUpdateRequest,
)
from app.domain.categories import validate_category_name, validate_category_slug
from app.domain.i18n import DEFAULT_LOCALE, validate_locale
from app.domain.moderation import classify_image
from app.domain.pages import build_page_permalink, validate_page_slug
from app.domain.permalinks import build_permalink
from app.models.blog import (
    Blog,
    BlogInvitation,
    BlogInvitationStatus,
    BlogMembership,
    BlogRole,
    BlogVisibility,
)
from app.models.blog_config import BlogConfig
from app.models.category import Category
from app.models.follow import BlogFollow
from app.models.page import Page
from app.models.post import Post, PostStatus
from app.models.post_link import post_links
from app.models.post_media import post_media
from app.models.post_note import post_notes
from app.models.user import User

router = APIRouter()


class BlogCreateRequest(BaseModel):
    slug: str
    title: str
    default_locale: str = DEFAULT_LOCALE
    subtitle: str | None = None
    description: str | None = None
    visibility: BlogVisibility = BlogVisibility.PUBLIC
    # CLAUDE.md #4: il frontend pre-compila questo campo con lo username di
    # chi crea il blog (resta modificabile) — vedi _resolve_author_display_name
    # in app/api/v1/posts.py per come si combina con l'alias di membership e
    # con la preferenza di profilo quando è vuoto.
    default_author_display_name: str | None = None


class BlogUpdateRequest(BaseModel):
    title: str | None = None
    # "" azzera (torna a nessun sottotitolo/descrizione), assente lascia
    # invariato — stesso schema di default_author_display_name.
    subtitle: str | None = None
    description: str | None = None
    visibility: BlogVisibility | None = None
    allow_anonymous_comments: bool | None = None
    # todo/EDITOR.md: @menzioni nei post trasformate in link (attive di default).
    mentions_enabled: bool | None = None
    # CLAUDE.md #1: pagine statiche del blog, opt-in e disattive di default.
    static_pages_enabled: bool | None = None
    # "" per tornare al default (username di chi scrive), qualsiasi altro
    # valore lo imposta; assente lascia invariato — stesso schema di
    # Post.cover_image_url in PATCH /posts/{id}.
    default_author_display_name: str | None = None


class BlogOut(BaseModel):
    id: uuid.UUID
    slug: str
    title: str
    subtitle: str | None
    description: str | None
    visibility: BlogVisibility
    custom_domain: str | None
    allow_anonymous_comments: bool
    mentions_enabled: bool
    static_pages_enabled: bool
    default_locale: str
    default_author_display_name: str | None
    # CLAUDE.md #8: presente solo per il proprietario stesso (usato lato
    # frontend per calcolare `isOwner`) — chiunque altro lo riceve a `null`,
    # perché è l'unico campo di Blog che punterebbe direttamente all'id
    # dell'utente reale dietro un blog che può mostrarsi con un alias
    # (Blog.default_author_display_name). Vedi _to_blog_out.
    owner_id: uuid.UUID | None
    created_at: datetime

    model_config = {"from_attributes": True}


class FollowerOut(BaseModel):
    username: str

    model_config = {"from_attributes": True}


async def _get_blog_or_404(session: AsyncSession, slug: str) -> Blog:
    result = await session.execute(select(Blog).where(Blog.slug == slug))
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    return blog


async def _require_blog_write_access(session: AsyncSession, user: User, blog: Blog) -> None:
    if not await can_write_posts(session, user_id=user.id, blog=blog):
        raise HTTPException(
            status.HTTP_403_FORBIDDEN,
            "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
        )


async def _require_blog_viewable(
    session: AsyncSession, user: User | None, blog: Blog
) -> None:
    """todo/BLOG.md #2: un blog `members`/`private` non deve rivelare nulla di
    sé a chi non può vederlo — 404 come se non esistesse, non 403."""
    if not await can_view_blog(session, user_id=user.id if user else None, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")


def _to_blog_out(blog: Blog, viewer: User | None) -> BlogOut:
    """CLAUDE.md #8: `owner_id` è l'unico campo di Blog che punta all'id
    reale di chi lo gestisce — nascosto a chiunque non sia il proprietario
    stesso, per non permettere di correlare un blog che usa un alias
    (Blog.default_author_display_name) con l'identità reale dietro di esso."""
    out = BlogOut.model_validate(blog)
    if viewer is None or viewer.id != blog.owner_id:
        out.owner_id = None
    return out


@router.post("", response_model=BlogOut, status_code=status.HTTP_201_CREATED)
async def create_blog(
    payload: BlogCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Blog:
    try:
        validate_blog_slug(payload.slug)
        validate_locale(payload.default_locale)
        if payload.subtitle:
            validate_blog_subtitle(payload.subtitle)
        if payload.description:
            validate_blog_description(payload.description)
        await assert_can_create_blog(session, current_user.id)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(select(Blog).where(Blog.slug == payload.slug))
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso.")

    blog = Blog(
        slug=payload.slug,
        title=payload.title,
        subtitle=(payload.subtitle or None),
        description=(payload.description or None),
        visibility=payload.visibility,
        default_locale=payload.default_locale,
        default_author_display_name=(payload.default_author_display_name or None),
        owner_id=current_user.id,
    )
    session.add(blog)
    await session.commit()
    await session.refresh(blog)
    return blog


@router.get("/mine", response_model=list[BlogOut])
async def list_my_blogs(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Blog]:
    """Solo i blog di proprietà. I blog su cui l'utente ha una membership
    (co-autore/mediatore/...) sono su `GET /api/v1/blogs/member-of`."""
    result = await session.execute(select(Blog).where(Blog.owner_id == current_user.id))
    return list(result.scalars().all())


class MembershipBlogOut(BaseModel):
    blog: BlogOut
    role: BlogRole
    author_display_name: str | None

    model_config = {"from_attributes": True}


@router.get("/member-of", response_model=list[MembershipBlogOut])
async def list_blogs_i_belong_to(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MembershipBlogOut]:
    """Blog altrui su cui l'utente corrente ha una membership attiva
    (accettando un invito, vedi sotto)."""
    result = await session.execute(
        select(BlogMembership, Blog)
        .join(Blog, Blog.id == BlogMembership.blog_id)
        .where(BlogMembership.user_id == current_user.id)
        .order_by(Blog.title)
    )
    return [
        MembershipBlogOut(
            blog=BlogOut.model_validate(blog),
            role=membership.role,
            author_display_name=membership.author_display_name,
        )
        for membership, blog in result.all()
    ]


class InvitationOut(BaseModel):
    id: uuid.UUID
    blog_slug: str
    blog_title: str
    role: BlogRole
    status: BlogInvitationStatus
    invited_username: str
    invited_by_username: str
    created_at: datetime
    responded_at: datetime | None

    model_config = {"from_attributes": True}


def _invitation_out(inv: BlogInvitation) -> InvitationOut:
    return InvitationOut(
        id=inv.id,
        blog_slug=inv.blog.slug,
        blog_title=inv.blog.title,
        role=inv.role,
        status=inv.status,
        invited_username=inv.invited_user.username,
        invited_by_username=inv.invited_by.username,
        created_at=inv.created_at,
        responded_at=inv.responded_at,
    )


_INVITATION_LOADS = (
    selectinload(BlogInvitation.blog),
    selectinload(BlogInvitation.invited_user),
    selectinload(BlogInvitation.invited_by),
)


@router.get("/received-invitations", response_model=list[InvitationOut])
async def list_received_invitations(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[InvitationOut]:
    """Inviti a collaborare ricevuti dall'utente corrente, ancora in attesa di
    risposta (todo/BLOG.md #3)."""
    result = await session.execute(
        select(BlogInvitation)
        .where(
            BlogInvitation.invited_user_id == current_user.id,
            BlogInvitation.status == BlogInvitationStatus.PENDING,
        )
        .options(*_INVITATION_LOADS)
        .order_by(BlogInvitation.created_at.desc())
    )
    return [_invitation_out(inv) for inv in result.scalars().all()]


async def _get_received_invitation_or_404(
    session: AsyncSession, invitation_id: uuid.UUID, user: User
) -> BlogInvitation:
    result = await session.execute(
        select(BlogInvitation)
        .where(BlogInvitation.id == invitation_id)
        .options(*_INVITATION_LOADS)
    )
    inv = result.scalar_one_or_none()
    if inv is None or inv.invited_user_id != user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Invito non trovato.")
    if inv.status != BlogInvitationStatus.PENDING:
        raise HTTPException(status.HTTP_409_CONFLICT, "Invito già gestito.")
    return inv


@router.post("/received-invitations/{invitation_id}/accept", response_model=InvitationOut)
async def accept_invitation(
    invitation_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> InvitationOut:
    inv = await _get_received_invitation_or_404(session, invitation_id, current_user)

    existing = await get_membership(session, user_id=current_user.id, blog_id=inv.blog_id)
    if existing is None:
        session.add(
            BlogMembership(user_id=current_user.id, blog_id=inv.blog_id, role=inv.role)
        )
    else:
        # già membro (es. invito duplicato via altra strada): allinea il ruolo.
        existing.role = inv.role
    inv.status = BlogInvitationStatus.ACCEPTED
    inv.responded_at = datetime.now(timezone.utc)
    await session.commit()
    await session.refresh(inv)
    return _invitation_out(inv)


@router.post("/received-invitations/{invitation_id}/decline", response_model=InvitationOut)
async def decline_invitation(
    invitation_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> InvitationOut:
    inv = await _get_received_invitation_or_404(session, invitation_id, current_user)
    inv.status = BlogInvitationStatus.DECLINED
    inv.responded_at = datetime.now(timezone.utc)
    await session.commit()
    await session.refresh(inv)
    return _invitation_out(inv)


@router.get("/{slug}", response_model=BlogOut)
async def get_blog(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> BlogOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    return _to_blog_out(blog, current_user)


@router.patch("/{slug}", response_model=BlogOut)
async def update_blog(
    slug: str,
    payload: BlogUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Blog:
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può modificare il blog.")

    try:
        if payload.subtitle:
            validate_blog_subtitle(payload.subtitle)
        if payload.description:
            validate_blog_description(payload.description)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if payload.title is not None:
        blog.title = payload.title
    if payload.subtitle is not None:
        blog.subtitle = payload.subtitle or None
    if payload.description is not None:
        blog.description = payload.description or None
    if payload.visibility is not None:
        blog.visibility = payload.visibility
    if payload.allow_anonymous_comments is not None:
        blog.allow_anonymous_comments = payload.allow_anonymous_comments
    if payload.mentions_enabled is not None:
        blog.mentions_enabled = payload.mentions_enabled
    if payload.static_pages_enabled is not None:
        blog.static_pages_enabled = payload.static_pages_enabled
    if payload.default_author_display_name is not None:
        blog.default_author_display_name = payload.default_author_display_name or None

    await session.commit()
    await session.refresh(blog)
    return blog


@router.post("/{slug}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def follow_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    existing = await session.execute(
        select(BlogFollow).where(
            BlogFollow.follower_id == current_user.id, BlogFollow.blog_id == blog.id
        )
    )
    if existing.scalar_one_or_none() is None:
        session.add(BlogFollow(follower_id=current_user.id, blog_id=blog.id))
        await session.commit()


@router.delete("/{slug}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def unfollow_blog(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    existing = await session.execute(
        select(BlogFollow).where(
            BlogFollow.follower_id == current_user.id, BlogFollow.blog_id == blog.id
        )
    )
    follow = existing.scalar_one_or_none()
    if follow is not None:
        await session.delete(follow)
        await session.commit()


@router.get("/{slug}/followers", response_model=list[FollowerOut])
async def list_blog_followers(slug: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    blog = await _get_blog_or_404(session, slug)
    result = await session.execute(
        select(User).join(BlogFollow, BlogFollow.follower_id == User.id).where(BlogFollow.blog_id == blog.id)
    )
    return list(result.scalars().all())


@router.get("/{slug}/config")
async def get_blog_config(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict[str, Any]:
    """Configurazione di presentazione (palette/tipografia/layout). Serve a
    renderizzare la pagina pubblica del blog; segue la visibilità del blog
    (todo/BLOG.md #2). Se il proprietario non ha ancora salvato nulla,
    ritorna il default della piattaforma."""
    result = await session.execute(
        select(Blog).where(Blog.slug == slug).options(selectinload(Blog.config))
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    await _require_blog_viewable(session, current_user, blog)
    return blog.config.config if blog.config is not None else DEFAULT_BLOG_CONFIG


@router.put("/{slug}/config")
async def update_blog_config(
    slug: str,
    payload: dict[str, Any],
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict[str, Any]:
    result = await session.execute(
        select(Blog).where(Blog.slug == slug).options(selectinload(Blog.config))
    )
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    if blog.owner_id != current_user.id:
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Solo il proprietario può modificare la configurazione.")

    try:
        validate_blog_config(payload)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if blog.config is None:
        blog.config = BlogConfig(blog_id=blog.id, config=payload)
        session.add(blog.config)
    else:
        blog.config.config = payload

    await session.commit()
    return payload


class MediaOut(BaseModel):
    url: str
    # Risultato della moderazione automatica (app/domain/moderation.py):
    # False anche se il servizio è disattivato/irraggiungibile — mai bloccante.
    is_sensitive: bool


@router.post("/{slug}/media", response_model=MediaOut, status_code=status.HTTP_201_CREATED)
async def upload_blog_media(
    slug: str,
    file: UploadFile,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MediaOut:
    """Immagine da incorporare nel Markdown di un post (CLAUDE.md #4:
    s3://{bucket}/{site_slug}/userdata/{user}/{blog}/media/...). Richiede
    accesso in scrittura al blog (proprietario/autore/co-autore)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)

    content = await file.read()
    try:
        object_key = upload_media(
            user_id=blog.owner_id,
            blog_id=blog.id,
            content=content,
            content_type=file.content_type or "",
        )
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    is_sensitive = await classify_image(content, file.filename or "image", file.content_type or "")
    return MediaOut(url=content_public_url(object_key), is_sensitive=is_sensitive)


class MentionableUserOut(BaseModel):
    username: str
    display_name: str | None

    model_config = {"from_attributes": True}


@router.get("/{slug}/mentionable-users", response_model=list[MentionableUserOut])
async def list_mentionable_users(
    slug: str,
    q: str = "",
    limit: int = 8,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MentionableUserOut]:
    """Suggerimenti per l'autocomplete delle @menzioni nell'editor
    (todo/EDITOR.md): proprietario, collaboratori e follower del blog il cui
    username o nome pubblico inizia/contiene `q`. Richiede accesso in scrittura
    al blog. Se le menzioni sono disattivate sul blog, ritorna lista vuota."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    if not blog.mentions_enabled:
        return []

    limit = min(max(limit, 1), 25)
    prefix = q.strip().lstrip("@").lower()

    related_ids = select(BlogMembership.user_id).where(BlogMembership.blog_id == blog.id).union(
        select(BlogFollow.follower_id).where(BlogFollow.blog_id == blog.id),
        select(Blog.owner_id).where(Blog.id == blog.id),
    )
    stmt = select(User).where(User.id.in_(related_ids))
    if prefix:
        like = f"%{prefix}%"
        stmt = stmt.where(
            func.lower(User.username).like(f"{prefix}%")
            | func.lower(func.coalesce(User.display_name, "")).like(like)
        )
    stmt = stmt.order_by(User.username).limit(limit)
    result = await session.execute(stmt)
    return [
        MentionableUserOut(username=u.username, display_name=u.display_name)
        for u in result.scalars().all()
    ]


class CategoryCreateRequest(BaseModel):
    name: str
    slug: str


class CategoryUpdateRequest(BaseModel):
    name: str | None = None
    slug: str | None = None


class CategoryOut(BaseModel):
    id: uuid.UUID
    name: str
    slug: str

    model_config = {"from_attributes": True}


@router.get("/{slug}/categories", response_model=list[CategoryOut])
async def list_categories(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Category]:
    """La tassonomia di un blog serve a orientarsi tra i contenuti (CLAUDE.md);
    segue la visibilità del blog (todo/BLOG.md #2)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    result = await session.execute(
        select(Category).where(Category.blog_id == blog.id).order_by(Category.name)
    )
    return list(result.scalars().all())


class BibliographyCitationOut(BaseModel):
    post_title: str
    post_slug: str
    permalink: str
    locale: str
    idx: int


class BibliographyEntryOut(BaseModel):
    content: str
    citations: list[BibliographyCitationOut]


@router.get("/{slug}/bibliography", response_model=list[BibliographyEntryOut])
async def get_blog_bibliography(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[BibliographyEntryOut]:
    """todo/EDITOR.md: bibliografia automatica del blog — tutte le note a piè
    di pagina dei post pubblicati, raggruppate per testo identico e con
    l'elenco dei post che le citano. Segue la visibilità del blog."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)

    rows = await session.execute(
        select(Post, post_notes.c.idx, post_notes.c.content)
        .join(post_notes, post_notes.c.post_id == Post.id)
        .where(
            Post.blog_id == blog.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
        )
        .order_by(Post.published_at.desc(), post_notes.c.idx.asc())
    )

    entries: dict[str, BibliographyEntryOut] = {}
    for post, idx, content in rows.all():
        key = " ".join(content.split()).casefold()
        entry = entries.get(key)
        if entry is None:
            entry = BibliographyEntryOut(content=content, citations=[])
            entries[key] = entry
        entry.citations.append(
            BibliographyCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                idx=idx,
            )
        )
    return list(entries.values())


class ContentCitationOut(BaseModel):
    post_title: str
    post_slug: str
    permalink: str
    locale: str
    used_at: datetime | None


class MediaBibliographyEntryOut(BaseModel):
    url: str
    alt_text: str
    categories: list[str]
    citations: list[ContentCitationOut]


@router.get("/{slug}/media-bibliography", response_model=list[MediaBibliographyEntryOut])
async def get_blog_media_bibliography(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MediaBibliographyEntryOut]:
    """CLAUDE.md #4: come la bibliografia delle note (sopra), ma per i media
    (oggi solo immagini) citati nel corpo dei post pubblicati — raggruppati
    per URL identico, con l'elenco dei post che li usano e la data di
    pubblicazione di ciascuno. Segue la visibilità del blog."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)

    rows = await session.execute(
        select(Post, post_media.c.alt_text, post_media.c.categories, post_media.c.url)
        .join(post_media, post_media.c.post_id == Post.id)
        .where(
            Post.blog_id == blog.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
        )
        .order_by(Post.published_at.desc(), post_media.c.position.asc())
    )

    entries: dict[str, MediaBibliographyEntryOut] = {}
    for post, alt_text, categories, url in rows.all():
        entry = entries.get(url)
        if entry is None:
            entry = MediaBibliographyEntryOut(url=url, alt_text=alt_text, categories=categories, citations=[])
            entries[url] = entry
        entry.citations.append(
            ContentCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                used_at=post.published_at,
            )
        )
    return list(entries.values())


class LinkBibliographyEntryOut(BaseModel):
    url: str
    link_text: str
    citations: list[ContentCitationOut]


@router.get("/{slug}/links-bibliography", response_model=list[LinkBibliographyEntryOut])
async def get_blog_links_bibliography(
    slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[LinkBibliographyEntryOut]:
    """CLAUDE.md #4: come la bibliografia dei media sopra, ma per i link
    citati nel corpo dei post pubblicati."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)

    rows = await session.execute(
        select(Post, post_links.c.link_text, post_links.c.url)
        .join(post_links, post_links.c.post_id == Post.id)
        .where(
            Post.blog_id == blog.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
        )
        .order_by(Post.published_at.desc(), post_links.c.position.asc())
    )

    entries: dict[str, LinkBibliographyEntryOut] = {}
    for post, link_text, url in rows.all():
        entry = entries.get(url)
        if entry is None:
            entry = LinkBibliographyEntryOut(url=url, link_text=link_text, citations=[])
            entries[url] = entry
        entry.citations.append(
            ContentCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                used_at=post.published_at,
            )
        )
    return list(entries.values())


@router.post("/{slug}/categories", response_model=CategoryOut, status_code=status.HTTP_201_CREATED)
async def create_category(
    slug: str,
    payload: CategoryCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Category:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)

    try:
        validate_category_name(payload.name)
        validate_category_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Category).where(Category.blog_id == blog.id, Category.slug == payload.slug)
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una categoria con questo slug.")

    category = Category(blog_id=blog.id, name=payload.name, slug=payload.slug)
    session.add(category)
    await session.commit()
    await session.refresh(category)
    return category


async def _get_category_or_404(session: AsyncSession, blog_id: uuid.UUID, category_id: uuid.UUID) -> Category:
    category = await session.get(Category, category_id)
    if category is None or category.blog_id != blog_id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Categoria non trovata.")
    return category


@router.patch("/{slug}/categories/{category_id}", response_model=CategoryOut)
async def update_category(
    slug: str,
    category_id: uuid.UUID,
    payload: CategoryUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Category:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    category = await _get_category_or_404(session, blog.id, category_id)

    try:
        if payload.name is not None:
            validate_category_name(payload.name)
        if payload.slug is not None:
            validate_category_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if payload.slug is not None and payload.slug != category.slug:
        existing = await session.execute(
            select(Category).where(Category.blog_id == blog.id, Category.slug == payload.slug)
        )
        if existing.scalar_one_or_none() is not None:
            raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una categoria con questo slug.")
        category.slug = payload.slug

    if payload.name is not None:
        category.name = payload.name

    await session.commit()
    await session.refresh(category)
    return category


@router.delete("/{slug}/categories/{category_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_category(
    slug: str,
    category_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """I post con questa categoria non vengono cancellati, restano solo
    senza categoria (FK ondelete SET NULL, vedi app/models/post.py)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    category = await _get_category_or_404(session, blog.id, category_id)

    await session.delete(category)
    await session.commit()


# ---- Pagine statiche del blog: feature opt-in (CLAUDE.md #1, todo/BLOG.md).
# Blog.static_pages_enabled, disattiva di default — sempre attiva invece per
# le pagine di piattaforma (app/api/v1/pages.py). Niente tag/categorie/
# pubblicazioni su queste pagine: solo titolo/slug/lingua/contenuto.


def _require_static_pages_enabled(blog: Blog) -> None:
    if not blog.static_pages_enabled:
        raise HTTPException(
            status.HTTP_403_FORBIDDEN, "Le pagine statiche non sono attive per questo blog."
        )


def _to_page_out(blog: Blog, page: Page) -> PageOut:
    out = PageOut.model_validate(page)
    out.permalink = build_page_permalink(blog.slug, page)
    out.mentions_enabled = blog.mentions_enabled
    return out


async def _get_blog_page_or_404(session: AsyncSession, blog_id: uuid.UUID, page_id: uuid.UUID) -> Page:
    page = await session.get(Page, page_id)
    if page is None or page.blog_id != blog_id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    return page


@router.get("/{slug}/pages", response_model=list[PageOut])
async def list_blog_pages(
    slug: str,
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[PageOut]:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    stmt = select(Page).where(Page.blog_id == blog.id, Page.locale == locale)
    can_write = current_user is not None and await can_write_posts(
        session, user_id=current_user.id, blog=blog
    )
    if not can_write:
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    return [_to_page_out(blog, page) for page in result.scalars().all()]


@router.get("/{slug}/pages/by-id/{page_id}", response_model=PageOut)
async def get_blog_page_by_id(
    slug: str,
    page_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Per l'editor di dashboard (bozza inclusa): richiede accesso in
    scrittura al blog, non la sola visibilità pubblica — a differenza di
    ``GET /{slug}/pages/{page_slug}`` sotto, pensato per la risoluzione del
    permalink pubblico. Registrata prima di quella rotta perché "by-id" non
    collida con `{page_slug}`."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    page = await _get_blog_page_or_404(session, blog.id, page_id)
    return _to_page_out(blog, page)


@router.get("/{slug}/pages/{page_slug}", response_model=PageOut)
async def get_blog_page(
    slug: str,
    page_slug: str,
    locale: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Pubblico: risolve il permalink /{blog_slug}/pagina/{page_slug} (solo
    pagine pubblicate, a meno che il chiamante non abbia accesso in scrittura
    sul blog — vedi app/domain/pages.py)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_viewable(session, current_user, blog)
    stmt = select(Page).where(Page.blog_id == blog.id, Page.slug == page_slug, Page.locale == locale)
    can_write = current_user is not None and await can_write_posts(
        session, user_id=current_user.id, blog=blog
    )
    if not can_write:
        stmt = stmt.where(Page.is_published.is_(True))
    result = await session.execute(stmt)
    page = result.scalar_one_or_none()
    if page is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Pagina non trovata.")
    return _to_page_out(blog, page)


@router.post("/{slug}/pages", response_model=PageOut, status_code=status.HTTP_201_CREATED)
async def create_blog_page(
    slug: str,
    payload: PageCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    _require_static_pages_enabled(blog)

    try:
        validate_locale(payload.locale)
        validate_page_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Page).where(
            Page.blog_id == blog.id, Page.slug == payload.slug, Page.locale == payload.locale
        )
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso per questa lingua.")

    page = Page(
        blog_id=blog.id,
        slug=payload.slug,
        locale=payload.locale,
        title=payload.title,
        content=payload.content,
        is_published=payload.is_published,
        updated_by_id=current_user.id,
    )
    session.add(page)
    await session.commit()
    await session.refresh(page)
    return _to_page_out(blog, page)


@router.post(
    "/{slug}/pages/{page_id}/translations", response_model=PageOut, status_code=status.HTTP_201_CREATED
)
async def add_blog_page_translation(
    slug: str,
    page_id: uuid.UUID,
    payload: PageTranslationRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    _require_static_pages_enabled(blog)
    original = await _get_blog_page_or_404(session, blog.id, page_id)

    try:
        validate_locale(payload.locale)
        validate_page_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Page).where(
            Page.translation_group_id == original.translation_group_id, Page.locale == payload.locale
        )
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Traduzione già presente per questa lingua.")

    translation = Page(
        blog_id=blog.id,
        slug=payload.slug,
        locale=payload.locale,
        translation_group_id=original.translation_group_id,
        title=payload.title,
        content=payload.content,
        is_published=payload.is_published,
        updated_by_id=current_user.id,
    )
    session.add(translation)
    await session.commit()
    await session.refresh(translation)
    return _to_page_out(blog, translation)


@router.get("/{slug}/pages/{page_id}/translations", response_model=list[PageTranslationSummaryOut])
async def list_blog_page_translations(
    slug: str,
    page_id: uuid.UUID,
    session: AsyncSession = Depends(get_session),
) -> list[Page]:
    """Per il selettore di lingua lato frontend, stesso pattern di
    app/api/v1/pages.py::list_page_translations (e di
    app/api/v1/posts.py::list_post_translations) — solo le pubblicate."""
    blog = await _get_blog_or_404(session, slug)
    original = await _get_blog_page_or_404(session, blog.id, page_id)
    result = await session.execute(
        select(Page).where(Page.translation_group_id == original.translation_group_id)
    )
    return [p for p in result.scalars().all() if p.is_published]


@router.patch("/{slug}/pages/{page_id}", response_model=PageOut)
async def update_blog_page(
    slug: str,
    page_id: uuid.UUID,
    payload: PageUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PageOut:
    """Modifica/rimozione restano possibili anche a feature disattivata
    (`static_pages_enabled=False` blocca solo la creazione di pagine/
    traduzioni nuove, non la gestione di quelle esistenti)."""
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    page = await _get_blog_page_or_404(session, blog.id, page_id)

    try:
        if payload.slug is not None:
            validate_page_slug(payload.slug)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if payload.slug is not None and payload.slug != page.slug:
        existing = await session.execute(
            select(Page).where(
                Page.blog_id == blog.id, Page.slug == payload.slug, Page.locale == page.locale
            )
        )
        if existing.scalar_one_or_none() is not None:
            raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso per questa lingua.")
        page.slug = payload.slug
    if payload.title is not None:
        page.title = payload.title
    if payload.content is not None:
        page.content = payload.content
    if payload.is_published is not None:
        page.is_published = payload.is_published
    page.updated_by_id = current_user.id

    await session.commit()
    await session.refresh(page)
    return _to_page_out(blog, page)


@router.delete("/{slug}/pages/{page_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_blog_page(
    slug: str,
    page_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _get_blog_or_404(session, slug)
    await _require_blog_write_access(session, current_user, blog)
    page = await _get_blog_page_or_404(session, blog.id, page_id)

    await session.delete(page)
    await session.commit()


# ---- Collaboratori: membership e inviti (todo/BLOG.md #3) -----------------

# Il todo limita gli inviti a co-autore e mediatore; autore/revisore restano
# assegnabili solo per via diretta a DB, non da questa interfaccia.
INVITABLE_ROLES = {BlogRole.CO_AUTORE, BlogRole.MEDIATORE}


class MemberOut(BaseModel):
    user_id: uuid.UUID
    username: str
    role: BlogRole
    author_display_name: str | None
    created_at: datetime

    model_config = {"from_attributes": True}


class InvitationCreateRequest(BaseModel):
    username: str
    role: BlogRole


class MemberRoleUpdateRequest(BaseModel):
    role: BlogRole


class MyMembershipUpdateRequest(BaseModel):
    # "" azzera (torna alla precedenza: default del blog → alias profilo →
    # username); assente lascia invariato.
    author_display_name: str | None = None


async def _require_blog_owner(session: AsyncSession, user: User, slug: str) -> Blog:
    blog = await _get_blog_or_404(session, slug)
    if blog.owner_id != user.id:
        raise HTTPException(
            status.HTTP_403_FORBIDDEN, "Solo il proprietario può gestire i collaboratori."
        )
    return blog


@router.get("/{slug}/members", response_model=list[MemberOut])
async def list_blog_members(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MemberOut]:
    blog = await _require_blog_owner(session, current_user, slug)
    result = await session.execute(
        select(BlogMembership, User)
        .join(User, User.id == BlogMembership.user_id)
        .where(BlogMembership.blog_id == blog.id)
        .order_by(User.username)
    )
    return [
        MemberOut(
            user_id=m.user_id,
            username=u.username,
            role=m.role,
            author_display_name=m.author_display_name,
            created_at=m.created_at,
        )
        for m, u in result.all()
    ]


@router.patch("/{slug}/members/{user_id}", response_model=MemberOut)
async def update_blog_member(
    slug: str,
    user_id: uuid.UUID,
    payload: MemberRoleUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MemberOut:
    blog = await _require_blog_owner(session, current_user, slug)
    if payload.role not in INVITABLE_ROLES:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST,
            "Ruolo assegnabile solo co_autore o mediatore da questa interfaccia.",
        )
    membership = await get_membership(session, user_id=user_id, blog_id=blog.id)
    if membership is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Collaboratore non trovato.")
    membership.role = payload.role
    await session.commit()
    user = await session.get(User, user_id)
    assert user is not None
    return MemberOut(
        user_id=membership.user_id,
        username=user.username,
        role=membership.role,
        author_display_name=membership.author_display_name,
        created_at=membership.created_at,
    )


@router.delete("/{slug}/members/{user_id}", status_code=status.HTTP_204_NO_CONTENT)
async def remove_blog_member(
    slug: str,
    user_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _require_blog_owner(session, current_user, slug)
    membership = await get_membership(session, user_id=user_id, blog_id=blog.id)
    if membership is not None:
        await session.delete(membership)
        await session.commit()


@router.patch("/{slug}/my-membership", response_model=MembershipBlogOut)
async def update_my_membership(
    slug: str,
    payload: MyMembershipUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MembershipBlogOut:
    """Il collaboratore sceglie l'alias con cui firmare i post su questo blog
    (todo/BLOG.md #4)."""
    blog = await _get_blog_or_404(session, slug)
    membership = await get_membership(session, user_id=current_user.id, blog_id=blog.id)
    if membership is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Non sei un collaboratore di questo blog.")
    if payload.author_display_name is not None:
        membership.author_display_name = payload.author_display_name or None
    await session.commit()
    await session.refresh(membership)
    return MembershipBlogOut(
        blog=BlogOut.model_validate(blog),
        role=membership.role,
        author_display_name=membership.author_display_name,
    )


@router.get("/{slug}/invitations", response_model=list[InvitationOut])
async def list_blog_invitations(
    slug: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[InvitationOut]:
    blog = await _require_blog_owner(session, current_user, slug)
    result = await session.execute(
        select(BlogInvitation)
        .where(BlogInvitation.blog_id == blog.id)
        .options(*_INVITATION_LOADS)
        .order_by(BlogInvitation.created_at.desc())
    )
    return [_invitation_out(inv) for inv in result.scalars().all()]


@router.post(
    "/{slug}/invitations", response_model=InvitationOut, status_code=status.HTTP_201_CREATED
)
async def create_blog_invitation(
    slug: str,
    payload: InvitationCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> InvitationOut:
    blog = await _require_blog_owner(session, current_user, slug)
    if payload.role not in INVITABLE_ROLES:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST,
            "Si può invitare solo come co_autore o mediatore.",
        )

    invited = await session.execute(select(User).where(User.username == payload.username))
    invited_user = invited.scalar_one_or_none()
    if invited_user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    if invited_user.id == blog.owner_id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Sei già il proprietario del blog.")

    if await get_membership(session, user_id=invited_user.id, blog_id=blog.id) is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "L'utente è già un collaboratore.")

    existing = await session.execute(
        select(BlogInvitation).where(
            BlogInvitation.blog_id == blog.id,
            BlogInvitation.invited_user_id == invited_user.id,
        )
    )
    inv = existing.scalar_one_or_none()
    if inv is not None and inv.status == BlogInvitationStatus.PENDING:
        raise HTTPException(status.HTTP_409_CONFLICT, "C'è già un invito in attesa per questo utente.")

    if inv is None:
        inv = BlogInvitation(
            blog_id=blog.id,
            invited_user_id=invited_user.id,
            invited_by_id=current_user.id,
            role=payload.role,
        )
        session.add(inv)
    else:
        # riusa la riga di un invito rifiutato/revocato in precedenza
        inv.role = payload.role
        inv.invited_by_id = current_user.id
        inv.status = BlogInvitationStatus.PENDING
        inv.responded_at = None

    await session.commit()
    result = await session.execute(
        select(BlogInvitation).where(BlogInvitation.id == inv.id).options(*_INVITATION_LOADS)
    )
    return _invitation_out(result.scalar_one())


@router.delete("/{slug}/invitations/{invitation_id}", status_code=status.HTTP_204_NO_CONTENT)
async def revoke_blog_invitation(
    slug: str,
    invitation_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    blog = await _require_blog_owner(session, current_user, slug)
    inv = await session.get(BlogInvitation, invitation_id)
    if inv is None or inv.blog_id != blog.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Invito non trovato.")
    if inv.status == BlogInvitationStatus.PENDING:
        inv.status = BlogInvitationStatus.REVOKED
        inv.responded_at = datetime.now(timezone.utc)
        await session.commit()
