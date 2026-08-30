import logging
import uuid
from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.core.broker import publish_post_backup
from app.core.database import get_session
from app.domain.authorization import can_review_posts, can_write_posts, is_publicly_visible
from app.domain.i18n import validate_locale
from app.models.blog import Blog
from app.models.post import Post, PostStatus
from app.models.user import User

router = APIRouter()
logger = logging.getLogger(__name__)


class PostCreateRequest(BaseModel):
    slug: str
    title: str
    content: str
    author_display_name: str | None = None
    locale: str | None = None  # default: la lingua di default del blog
    cover_image_url: str | None = None


class PostTranslationRequest(BaseModel):
    slug: str
    locale: str
    title: str
    content: str
    author_display_name: str | None = None
    cover_image_url: str | None = None


class PostUpdateRequest(BaseModel):
    title: str | None = None
    content: str | None = None
    # stringa vuota per rimuovere la cover impostata in precedenza; assente
    # (None) per non toccarla — stesso schema di "campo opzionale" degli
    # altri, ma qui None ha un significato ambiguo (rimuovi vs non toccare)
    # che risolviamo trattando "" come "rimuovi" in update_post.
    cover_image_url: str | None = None


class PublishRequest(BaseModel):
    # se futuro: pianifica la pubblicazione invece di renderla effettiva subito
    published_at: datetime | None = None


class PostOut(BaseModel):
    id: uuid.UUID
    blog_id: uuid.UUID
    author_id: uuid.UUID
    author_display_name: str
    locale: str
    translation_group_id: uuid.UUID
    title: str
    slug: str
    content: str
    cover_image_url: str | None
    status: PostStatus
    published_at: datetime | None
    created_at: datetime

    model_config = {"from_attributes": True}


class TranslationSummaryOut(BaseModel):
    id: uuid.UUID
    locale: str
    slug: str
    status: PostStatus

    model_config = {"from_attributes": True}


async def _get_blog_or_404(session: AsyncSession, blog_slug: str) -> Blog:
    result = await session.execute(select(Blog).where(Blog.slug == blog_slug))
    blog = result.scalar_one_or_none()
    if blog is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Blog non trovato.")
    return blog


async def _get_post_or_404(session: AsyncSession, post_id: uuid.UUID) -> Post:
    post = await session.get(Post, post_id)
    if post is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    return post


async def _require_write_access(session: AsyncSession, user: User, blog: Blog) -> None:
    if not await can_write_posts(session, user_id=user.id, blog=blog):
        raise HTTPException(
            status.HTTP_403_FORBIDDEN,
            "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
        )


def _backup_to_s3(blog: Blog, post: Post) -> None:
    """Fire-and-forget: accoda il backup su S3. Il database resta la fonte di
    verità del post, quindi un problema di RabbitMQ/S3 qui non deve mai far
    fallire il salvataggio già andato a buon fine."""
    try:
        publish_post_backup(
            user_id=str(blog.owner_id),
            blog_id=str(blog.id),
            post_id=str(post.id),
            title=post.title,
            content=post.content,
            locale=post.locale,
        )
    except Exception:
        logger.warning("Impossibile accodare il backup S3 per il post %s", post.id, exc_info=True)


@router.post("/blogs/{blog_slug}/posts", response_model=PostOut, status_code=status.HTTP_201_CREATED)
async def create_post(
    blog_slug: str,
    payload: PostCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    blog = await _get_blog_or_404(session, blog_slug)
    await _require_write_access(session, current_user, blog)

    locale = payload.locale or blog.default_locale
    try:
        validate_locale(locale)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing = await session.execute(
        select(Post).where(Post.blog_id == blog.id, Post.slug == payload.slug, Post.locale == locale)
    )
    if existing.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso su questo blog per questa lingua.")

    post = Post(
        blog_id=blog.id,
        author_id=current_user.id,
        author_display_name=payload.author_display_name or current_user.username,
        locale=locale,
        title=payload.title,
        slug=payload.slug,
        content=payload.content,
        cover_image_url=payload.cover_image_url,
    )
    session.add(post)
    await session.commit()
    await session.refresh(post)
    _backup_to_s3(blog, post)
    return post


@router.post(
    "/posts/{post_id}/translations", response_model=PostOut, status_code=status.HTTP_201_CREATED
)
async def add_post_translation(
    post_id: uuid.UUID,
    payload: PostTranslationRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    original = await _get_post_or_404(session, post_id)
    blog = await session.get(Blog, original.blog_id)
    assert blog is not None
    await _require_write_access(session, current_user, blog)

    try:
        validate_locale(payload.locale)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    existing_locale = await session.execute(
        select(Post).where(
            Post.translation_group_id == original.translation_group_id, Post.locale == payload.locale
        )
    )
    if existing_locale.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Esiste già una traduzione per questa lingua.")

    existing_slug = await session.execute(
        select(Post).where(
            Post.blog_id == blog.id, Post.slug == payload.slug, Post.locale == payload.locale
        )
    )
    if existing_slug.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Slug già in uso su questo blog per questa lingua.")

    translation = Post(
        blog_id=blog.id,
        author_id=current_user.id,
        author_display_name=payload.author_display_name or current_user.username,
        translation_group_id=original.translation_group_id,
        locale=payload.locale,
        title=payload.title,
        slug=payload.slug,
        content=payload.content,
        cover_image_url=payload.cover_image_url,
    )
    session.add(translation)
    await session.commit()
    await session.refresh(translation)
    _backup_to_s3(blog, translation)
    return translation


@router.get("/posts/{post_id}/translations", response_model=list[TranslationSummaryOut])
async def list_post_translations(
    post_id: uuid.UUID, session: AsyncSession = Depends(get_session)
) -> list[Post]:
    """Per costruire un selettore di lingua lato frontend."""
    original = await _get_post_or_404(session, post_id)
    result = await session.execute(
        select(Post).where(Post.translation_group_id == original.translation_group_id)
    )
    return [p for p in result.scalars().all() if is_publicly_visible(p)]


@router.get("/blogs/{blog_slug}/posts", response_model=list[PostOut])
async def list_posts(
    blog_slug: str,
    locale: str | None = None,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[Post]:
    """Pubblico: solo i post effettivamente pubblicati (pubblicati e, se
    pianificati, con data raggiunta). Chi ha accesso in scrittura al blog
    (proprietario/autore/co-autore) vede anche bozze/in revisione/pianificati."""
    blog = await _get_blog_or_404(session, blog_slug)

    stmt = select(Post).where(Post.blog_id == blog.id)
    if locale is not None:
        stmt = stmt.where(Post.locale == locale)
    result = await session.execute(stmt)
    posts = list(result.scalars().all())

    has_write_access = current_user is not None and await can_write_posts(
        session, user_id=current_user.id, blog=blog
    )
    if has_write_access:
        return posts
    return [p for p in posts if is_publicly_visible(p)]


@router.get("/posts/{post_id}", response_model=PostOut)
async def get_post(
    post_id: uuid.UUID,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    post = await _get_post_or_404(session, post_id)
    if is_publicly_visible(post):
        return post

    # bozza/in revisione/pianificato: visibile solo a chi ha accesso in
    # scrittura al blog (non un 403 esplicito, per non rivelarne l'esistenza)
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    if current_user is None or not await can_write_posts(session, user_id=current_user.id, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    return post


@router.patch("/posts/{post_id}", response_model=PostOut)
async def update_post(
    post_id: uuid.UUID,
    payload: PostUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    post = await _get_post_or_404(session, post_id)
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    await _require_write_access(session, current_user, blog)

    if payload.title is not None:
        post.title = payload.title
    if payload.content is not None:
        post.content = payload.content
    if payload.cover_image_url is not None:
        post.cover_image_url = payload.cover_image_url or None

    await session.commit()
    await session.refresh(post)
    _backup_to_s3(blog, post)
    return post


@router.post("/posts/{post_id}/submit-for-review", response_model=PostOut)
async def submit_for_review(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    """Ruolo Revisore (CLAUDE.md #1): un autore manda il proprio draft in
    revisione invece di pubblicarlo direttamente."""
    post = await _get_post_or_404(session, post_id)
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    await _require_write_access(session, current_user, blog)

    if post.status != PostStatus.DRAFT:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Solo una bozza può essere mandata in revisione.")
    post.status = PostStatus.PENDING_REVIEW
    await session.commit()
    await session.refresh(post)
    return post


@router.post("/posts/{post_id}/return-to-draft", response_model=PostOut)
async def return_to_draft(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    """Il Revisore (o il proprietario) rimanda un post in pending_review
    all'autore per ulteriori modifiche, invece di approvarlo."""
    post = await _get_post_or_404(session, post_id)
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None

    if not await can_review_posts(session, user_id=current_user.id, blog=blog):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "Serve essere proprietario del blog o revisore.")
    if post.status != PostStatus.PENDING_REVIEW:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Il post non è in revisione.")

    post.status = PostStatus.DRAFT
    await session.commit()
    await session.refresh(post)
    return post


@router.post("/posts/{post_id}/publish", response_model=PostOut)
async def publish_post(
    post_id: uuid.UUID,
    payload: PublishRequest | None = None,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> Post:
    """Pubblica (subito o pianificato via `published_at` futuro). Consentito
    a proprietario/autore/co-autore da qualsiasi stato; un Revisore può
    approvare solo da pending_review (vedi return-to-draft per il rifiuto)."""
    post = await _get_post_or_404(session, post_id)
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None

    has_write_access = await can_write_posts(session, user_id=current_user.id, blog=blog)
    if not has_write_access:
        is_reviewer_approval = post.status == PostStatus.PENDING_REVIEW and await can_review_posts(
            session, user_id=current_user.id, blog=blog
        )
        if not is_reviewer_approval:
            raise HTTPException(
                status.HTTP_403_FORBIDDEN,
                "Serve essere proprietario/autore/co-autore, o revisore per approvare un post in revisione.",
            )

    scheduled_at = payload.published_at if payload else None
    if post.status == PostStatus.PUBLISHED and scheduled_at is None:
        # già pubblicato e nessun nuovo orario esplicito: no-op idempotente,
        # non tocca published_at
        return post

    post.status = PostStatus.PUBLISHED
    post.published_at = scheduled_at or datetime.now(timezone.utc)
    await session.commit()
    await session.refresh(post)
    return post
