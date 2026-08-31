import logging
import uuid
from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import delete, insert, select
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user, get_optional_current_user
from app.core.broker import publish_post_backup
from app.core.database import get_session
from app.domain.authorization import can_review_posts, can_write_posts, is_publicly_visible
from app.domain.i18n import validate_locale
from app.domain.permalinks import build_permalink, is_valid_permalink_date, permalink_date
from app.domain.tags import resolve_tags
from app.models.blog import Blog
from app.models.category import Category
from app.models.post import Post, PostStatus
from app.models.tag import Tag, post_tags
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
    # Esito della moderazione automatica ricevuto da POST /blogs/{slug}/media
    # al momento dell'upload (vedi app/domain/moderation.py) — non ricalcolato qui.
    cover_image_is_sensitive: bool = False
    # Tag del campo dedicato (vedi app/domain/tags.py); si sommano agli
    # eventuali #hashtag scritti nel testo, massimo 5 in tutto.
    tags: list[str] | None = None
    # Categoria (vedi app/domain/categories.py) — al più una, deve
    # appartenere allo stesso blog.
    category_id: uuid.UUID | None = None


class PostTranslationRequest(BaseModel):
    slug: str
    locale: str
    title: str
    content: str
    author_display_name: str | None = None
    cover_image_url: str | None = None
    cover_image_is_sensitive: bool = False
    tags: list[str] | None = None
    category_id: uuid.UUID | None = None


class PostUpdateRequest(BaseModel):
    title: str | None = None
    content: str | None = None
    # stringa vuota per rimuovere la cover impostata in precedenza; assente
    # (None) per non toccarla — stesso schema di "campo opzionale" degli
    # altri, ma qui None ha un significato ambiguo (rimuovi vs non toccare)
    # che risolviamo trattando "" come "rimuovi" in update_post.
    cover_image_url: str | None = None
    # assente: lascia invariato; ha senso solo insieme a un nuovo cover_image_url.
    cover_image_is_sensitive: bool | None = None
    # assente: lascia invariati i tag del campo dedicato; lista (anche vuota
    # []): la sostituisce. Gli #hashtag nel testo sono comunque ricalcolati
    # ad ogni modifica del contenuto, a prescindere da questo campo.
    tags: list[str] | None = None
    # qui `null` è un valore significativo (rimuove la categoria), diverso da
    # "campo assente" (non toccarla) — servirsi di model_fields_set in
    # update_post, non di un semplice "is not None".
    category_id: uuid.UUID | None = None


class PublishRequest(BaseModel):
    # se futuro: pianifica la pubblicazione invece di renderla effettiva subito
    published_at: datetime | None = None


class CategorySummaryOut(BaseModel):
    id: uuid.UUID
    name: str
    slug: str

    model_config = {"from_attributes": True}


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
    cover_image_is_sensitive: bool
    status: PostStatus
    published_at: datetime | None
    created_at: datetime
    # permalink leggibile /{blog_slug}/{YYYYMMDD}/{slug} (CLAUDE.md #2: niente
    # UUID negli URL pubblici) — non colonne del modello, calcolati da
    # _post_out() ad ogni risposta, serve perciò anche blog_slug qui.
    blog_slug: str
    permalink: str
    # manual_tags: solo quelli del campo dedicato (per ripresentarli in
    # modifica). tags: l'insieme effettivo (manual_tags + hashtag nel testo),
    # per la visualizzazione pubblica e le pagine/tendenze per tag.
    manual_tags: list[str]
    tags: list[str]
    category: CategorySummaryOut | None

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


async def _validate_category(session: AsyncSession, blog: Blog, category_id: uuid.UUID | None) -> None:
    if category_id is None:
        return
    category = await session.get(Category, category_id)
    if category is None or category.blog_id != blog.id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Categoria non valida per questo blog.")


async def _post_out(session: AsyncSession, post: Post, blog: Blog) -> PostOut:
    # ricalcolato da colonne scalari già in memoria (manual_tags, content),
    # non dalla relazione ORM `tags` — che richiederebbe un lazy load async
    # non sempre già eseguito da chi chiama (vedi _sync_post_tags per dove
    # quella relazione viene invece scritta). Stesso motivo per `category`:
    # un session.get() esplicito qui, non la relazione ORM su Post.
    _, effective_tags = resolve_tags(post.manual_tags, post.content)
    category = await session.get(Category, post.category_id) if post.category_id else None
    return PostOut(
        id=post.id,
        blog_id=post.blog_id,
        author_id=post.author_id,
        author_display_name=post.author_display_name,
        locale=post.locale,
        translation_group_id=post.translation_group_id,
        title=post.title,
        slug=post.slug,
        content=post.content,
        cover_image_url=post.cover_image_url,
        cover_image_is_sensitive=post.cover_image_is_sensitive,
        status=post.status,
        published_at=post.published_at,
        created_at=post.created_at,
        blog_slug=blog.slug,
        permalink=build_permalink(blog.slug, post),
        manual_tags=post.manual_tags,
        tags=effective_tags,
        category=CategorySummaryOut.model_validate(category) if category else None,
    )


async def _sync_post_tags(session: AsyncSession, post: Post, effective_tags: list[str]) -> None:
    """Materializza l'insieme effettivo di tag nella tabella d'associazione
    post_tags — non serve per PostOut (vedi sopra), solo per le query di
    aggregazione tra post (tendenze). Get-or-create per ogni tag.

    Passa deliberatamente dalla tabella `post_tags` (Core), non dalla
    relazione ORM `Post.tags`: assegnarla direttamente (`post.tags = ...`)
    fa scattare, per via del back_populates bidirezionale, un flush
    implicito nella semplice istruzione di assegnazione — che essendo
    sincrona (non awaited) non ha il contesto async necessario e fallisce.
    session.execute()/flush() qui sono invece sempre esplicitamente awaited."""
    tag_ids = []
    for name in effective_tags:
        result = await session.execute(select(Tag).where(Tag.name == name))
        tag = result.scalar_one_or_none()
        if tag is None:
            tag = Tag(name=name)
            session.add(tag)
            await session.flush()  # serve l'id del nuovo tag per l'insert sotto
        tag_ids.append(tag.id)

    # il post deve già esistere in DB perché post_tags referenzia posts.id
    # con FK: per un post appena creato lo garantisce l'autoflush dei
    # session.execute(select(...)) sopra (flushano anche il Post pending).
    await session.execute(delete(post_tags).where(post_tags.c.post_id == post.id))
    if tag_ids:
        await session.execute(
            insert(post_tags),
            [{"post_id": post.id, "tag_id": tag_id} for tag_id in tag_ids],
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
) -> PostOut:
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

    try:
        manual_tags, effective_tags = resolve_tags(payload.tags or [], payload.content)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    await _validate_category(session, blog, payload.category_id)

    post = Post(
        blog_id=blog.id,
        author_id=current_user.id,
        author_display_name=payload.author_display_name or blog.default_author_display_name or current_user.username,
        locale=locale,
        title=payload.title,
        slug=payload.slug,
        content=payload.content,
        cover_image_url=payload.cover_image_url,
        cover_image_is_sensitive=payload.cover_image_is_sensitive,
        manual_tags=manual_tags,
        category_id=payload.category_id,
    )
    session.add(post)
    await _sync_post_tags(session, post, effective_tags)
    await session.commit()
    await session.refresh(post, attribute_names=["created_at", "updated_at"])
    _backup_to_s3(blog, post)
    return await _post_out(session, post, blog)


@router.post(
    "/posts/{post_id}/translations", response_model=PostOut, status_code=status.HTTP_201_CREATED
)
async def add_post_translation(
    post_id: uuid.UUID,
    payload: PostTranslationRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
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

    try:
        manual_tags, effective_tags = resolve_tags(payload.tags or [], payload.content)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    # se non specificata esplicitamente, la traduzione eredita la categoria
    # dell'originale (stesso contenuto, presumibilmente stessa categoria)
    category_id = (
        payload.category_id if "category_id" in payload.model_fields_set else original.category_id
    )
    await _validate_category(session, blog, category_id)

    translation = Post(
        blog_id=blog.id,
        author_id=current_user.id,
        author_display_name=payload.author_display_name or blog.default_author_display_name or current_user.username,
        translation_group_id=original.translation_group_id,
        locale=payload.locale,
        title=payload.title,
        slug=payload.slug,
        content=payload.content,
        cover_image_url=payload.cover_image_url,
        cover_image_is_sensitive=payload.cover_image_is_sensitive,
        manual_tags=manual_tags,
        category_id=category_id,
    )
    session.add(translation)
    await _sync_post_tags(session, translation, effective_tags)
    await session.commit()
    await session.refresh(translation, attribute_names=["created_at", "updated_at"])
    _backup_to_s3(blog, translation)
    return await _post_out(session, translation, blog)


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
) -> list[PostOut]:
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
    if not has_write_access:
        posts = [p for p in posts if is_publicly_visible(p)]
    return [await _post_out(session, p, blog) for p in posts]


@router.get("/posts/{post_id}", response_model=PostOut)
async def get_post(
    post_id: uuid.UUID,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
    post = await _get_post_or_404(session, post_id)
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    if is_publicly_visible(post):
        return await _post_out(session, post, blog)

    # bozza/in revisione/pianificato: visibile solo a chi ha accesso in
    # scrittura al blog (non un 403 esplicito, per non rivelarne l'esistenza)
    if current_user is None or not await can_write_posts(session, user_id=current_user.id, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    return await _post_out(session, post, blog)


@router.get("/blogs/{blog_slug}/posts/{permalink_date_str}/{post_slug}", response_model=PostOut)
async def get_post_by_permalink(
    blog_slug: str,
    permalink_date_str: str,
    post_slug: str,
    current_user: User | None = Depends(get_optional_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
    """Risoluzione del permalink pubblico /{blog_slug}/{YYYYMMDD}/{post_slug}
    (CLAUDE.md #2): nessun UUID nell'URL. La data è quella di pubblicazione,
    o di creazione per l'anteprima di una bozza (vedi domain/permalinks.py)."""
    if not is_valid_permalink_date(permalink_date_str):
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Formato data non valido, atteso YYYYMMDD.")

    blog = await _get_blog_or_404(session, blog_slug)
    result = await session.execute(
        select(Post).where(Post.blog_id == blog.id, Post.slug == post_slug)
    )
    candidates = [
        p for p in result.scalars().all() if permalink_date(p).strftime("%Y%m%d") == permalink_date_str
    ]
    if not candidates:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    # Lo slug è unico solo per (blog, locale): due traduzioni diverse
    # potrebbero in teoria condividere slug+data. Caso raro, non impedito a
    # livello di vincolo DB — si preferisce la lingua di default del blog.
    post = next((p for p in candidates if p.locale == blog.default_locale), candidates[0])

    if not is_publicly_visible(post):
        if current_user is None or not await can_write_posts(session, user_id=current_user.id, blog=blog):
            raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    return await _post_out(session, post, blog)


@router.patch("/posts/{post_id}", response_model=PostOut)
async def update_post(
    post_id: uuid.UUID,
    payload: PostUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
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
        # nessuna nuova cover (rimossa): non ha senso restare "sensibile".
        post.cover_image_is_sensitive = bool(post.cover_image_url) and (
            payload.cover_image_is_sensitive or False
        )

    # i tag vanno ricalcolati se è cambiato il contenuto (gli #hashtag nel
    # testo potrebbero essere cambiati) o se il campo dedicato è stato
    # esplicitamente passato — altrimenti restano quelli già salvati.
    if payload.content is not None or payload.tags is not None:
        new_manual = payload.tags if payload.tags is not None else post.manual_tags
        try:
            manual_tags, effective_tags = resolve_tags(new_manual, post.content)
        except ValueError as exc:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        post.manual_tags = manual_tags
        await _sync_post_tags(session, post, effective_tags)

    if "category_id" in payload.model_fields_set:
        await _validate_category(session, blog, payload.category_id)
        post.category_id = payload.category_id

    await session.commit()
    await session.refresh(post, attribute_names=["created_at", "updated_at"])
    _backup_to_s3(blog, post)
    return await _post_out(session, post, blog)


@router.post("/posts/{post_id}/submit-for-review", response_model=PostOut)
async def submit_for_review(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
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
    return await _post_out(session, post, blog)


@router.post("/posts/{post_id}/return-to-draft", response_model=PostOut)
async def return_to_draft(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
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
    return await _post_out(session, post, blog)


@router.post("/posts/{post_id}/publish", response_model=PostOut)
async def publish_post(
    post_id: uuid.UUID,
    payload: PublishRequest | None = None,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> PostOut:
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
        return await _post_out(session, post, blog)

    post.status = PostStatus.PUBLISHED
    post.published_at = scheduled_at or datetime.now(timezone.utc)
    await session.commit()
    await session.refresh(post)
    return await _post_out(session, post, blog)
