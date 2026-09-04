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
from app.domain.authorization import (
    can_review_posts,
    can_view_blog,
    can_write_posts,
    get_membership,
    is_publicly_visible,
)
from app.domain.content_media import extract_links, extract_media
from app.domain.display_names import resolve_personal_display_name
from app.domain.i18n import validate_locale
from app.domain.notes import NoteInput, normalize_notes
from app.domain.permalinks import build_permalink, is_valid_permalink_date, permalink_date
from app.domain.tags import resolve_tags
from app.models.blog import Blog
from app.models.category import Category
from app.models.post import Post, PostStatus
from app.models.post_link import post_links
from app.models.post_media import post_media
from app.models.post_note import post_notes
from app.models.tag import Tag, post_tags
from app.models.user import User

router = APIRouter()
logger = logging.getLogger(__name__)


class NoteIn(BaseModel):
    idx: int
    content: str


class NoteOut(BaseModel):
    idx: int
    content: str


def _to_note_inputs(notes: list[NoteIn] | None) -> list[NoteInput]:
    return [NoteInput(idx=n.idx, content=n.content) for n in (notes or [])]


class PostCreateRequest(BaseModel):
    slug: str
    title: str
    content: str
    locale: str | None = None  # default: la lingua di default del blog
    cover_image_url: str | None = None
    # Esito della moderazione automatica ricevuto da POST /blogs/{slug}/media
    # al momento dell'upload (vedi app/domain/moderation.py) — non ricalcolato qui.
    cover_image_is_sensitive: bool = False
    # Categorie di avviso scelte manualmente dal modal stile Bluesky
    # (CLAUDE.md #3, vocabolario in app/domain/content_media.py). Non vuoto
    # forza anche cover_image_is_sensitive a True.
    cover_image_categories: list[str] = []
    # Tag del campo dedicato (vedi app/domain/tags.py); si sommano agli
    # eventuali #hashtag scritti nel testo, massimo 5 in tutto.
    tags: list[str] | None = None
    # Categoria (vedi app/domain/categories.py) — al più una, deve
    # appartenere allo stesso blog.
    category_id: uuid.UUID | None = None
    # Note a piè di pagina (todo/EDITOR.md): elenco strutturato, non nel
    # corpo. Nel `content` il riferimento è il marcatore `[idx](#nota-idx)`.
    notes: list[NoteIn] | None = None


class PostTranslationRequest(BaseModel):
    slug: str
    locale: str
    title: str
    content: str
    cover_image_url: str | None = None
    cover_image_is_sensitive: bool = False
    cover_image_categories: list[str] = []
    tags: list[str] | None = None
    category_id: uuid.UUID | None = None
    notes: list[NoteIn] | None = None


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
    # A differenza di cover_image_is_sensitive sopra, indipendente da un
    # nuovo cover_image_url: il modal di avviso sui contenuti (CLAUDE.md #3)
    # deve poter cambiare le categorie di un'immagine di copertina già
    # esistente. Assente: lascia invariate; lista (anche vuota `[]`): la
    # sostituisce — non vuota forza anche cover_image_is_sensitive a True.
    cover_image_categories: list[str] | None = None
    # assente: lascia invariati i tag del campo dedicato; lista (anche vuota
    # []): la sostituisce. Gli #hashtag nel testo sono comunque ricalcolati
    # ad ogni modifica del contenuto, a prescindere da questo campo.
    tags: list[str] | None = None
    # qui `null` è un valore significativo (rimuove la categoria), diverso da
    # "campo assente" (non toccarla) — servirsi di model_fields_set in
    # update_post, non di un semplice "is not None".
    category_id: uuid.UUID | None = None
    # assente: lascia invariate le note; lista (anche vuota `[]`): le sostituisce.
    notes: list[NoteIn] | None = None


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
    cover_image_categories: list[str]
    status: PostStatus
    published_at: datetime | None
    created_at: datetime
    # Vedi Post.is_hidden: True se un admin di piattaforma lo ha nascosto
    # (dashboard/moderazione), indipendentemente da `status`. L'autore lo
    # vede qui per sapere perché il post non è raggiungibile pubblicamente.
    is_hidden: bool
    # permalink leggibile /{blog_slug}/{YYYYMMDD}/{slug} (CLAUDE.md #2: niente
    # UUID negli URL pubblici) — non colonne del modello, calcolati da
    # _post_out() ad ogni risposta, serve perciò anche blog_slug qui.
    blog_slug: str
    permalink: str
    # todo/EDITOR.md: se il blog ha le @menzioni attive, il frontend le
    # trasforma in link al profilo citato al momento del rendering.
    mentions_enabled: bool
    # Note a piè di pagina (todo/EDITOR.md), ordinate per `idx`. Il frontend
    # le rende come elenco in fondo + tooltip sui marcatori nel testo.
    notes: list[NoteOut]
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


async def _require_blog_viewable(session: AsyncSession, user: User | None, blog: Blog) -> None:
    """todo/BLOG.md #2: i post di un blog `members`/`private` non sono
    raggiungibili da chi non può vedere il blog — 404, non 403."""
    if not await can_view_blog(session, user_id=user.id if user else None, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")


async def _require_write_access(session: AsyncSession, user: User, blog: Blog) -> None:
    if not await can_write_posts(session, user_id=user.id, blog=blog):
        raise HTTPException(
            status.HTTP_403_FORBIDDEN,
            "Serve essere proprietario del blog o avere ruolo autore/co-autore.",
        )


async def _resolve_author_display_name(session: AsyncSession, user: User, blog: Blog) -> str:
    """Nome pubblico dell'autore di un post (CLAUDE.md #1, todo/BLOG.md #4,
    todo/USERS.md #2). Scritto sulla colonna `Post.author_display_name` alla
    creazione/modifica del post, ma ricalcolato di nuovo ad ogni lettura in
    `_post_out` — così un alias di membership o di blog cambiato dopo la
    pubblicazione si riflette subito su tutti i post esistenti, non solo su
    quelli risalvati dall'autore. Non c'è un valore per singolo post indicato
    dal client.

    1. Alias dell'autore sulla propria membership di *questo* blog, se presente;
    2. nome pubblico predefinito del blog (`default_author_display_name`), se presente.
       Se uno di questi due esiste, è imposto: nessuna possibilità di override.
    3. altrimenti la preferenza dell'utente (`post_author_name_style`):
       nome e cognome, alias globale del profilo, o username (default).
    """
    membership = await get_membership(session, user_id=user.id, blog_id=blog.id)
    if membership is not None and membership.author_display_name:
        return membership.author_display_name
    if blog.default_author_display_name:
        return blog.default_author_display_name

    return resolve_personal_display_name(user)


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
    note_rows = await session.execute(
        select(post_notes.c.idx, post_notes.c.content)
        .where(post_notes.c.post_id == post.id)
        .order_by(post_notes.c.idx)
    )
    notes = [NoteOut(idx=idx, content=content) for idx, content in note_rows.all()]
    # Ricalcolato ad ogni lettura (non dalla colonna `author_display_name`,
    # che resta solo l'ultimo valore scritto): un alias di blog/membership
    # cambiato dopo la pubblicazione deve riflettersi subito su tutti i post
    # già scritti, non solo su quelli risalvati dall'autore (CLAUDE.md #1).
    author = await session.get(User, post.author_id)
    author_display_name = (
        await _resolve_author_display_name(session, author, blog)
        if author is not None
        else post.author_display_name
    )
    return PostOut(
        id=post.id,
        blog_id=post.blog_id,
        author_id=post.author_id,
        author_display_name=author_display_name,
        locale=post.locale,
        translation_group_id=post.translation_group_id,
        title=post.title,
        slug=post.slug,
        content=post.content,
        cover_image_url=post.cover_image_url,
        cover_image_is_sensitive=post.cover_image_is_sensitive,
        cover_image_categories=post.cover_image_categories,
        status=post.status,
        published_at=post.published_at,
        created_at=post.created_at,
        is_hidden=post.is_hidden,
        blog_slug=blog.slug,
        permalink=build_permalink(blog.slug, post),
        mentions_enabled=blog.mentions_enabled,
        notes=notes,
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


async def _sync_post_notes(session: AsyncSession, post: Post, notes: list[NoteInput]) -> None:
    """Riscrive le note del post nella tabella `post_notes` (Core, non una
    relazione ORM — stessa insidia async di `_sync_post_tags`). Sorgente di
    verità delle note è questo elenco strutturato, non il corpo del post."""
    # per un post appena creato serve l'id, che i default di colonna assegnano
    # solo al flush (un delete/insert Core non lo fa scattare da sé).
    if post.id is None:
        await session.flush()
    await session.execute(delete(post_notes).where(post_notes.c.post_id == post.id))
    if notes:
        await session.execute(
            insert(post_notes),
            [{"post_id": post.id, "idx": n.idx, "content": n.content} for n in notes],
        )


async def _sync_post_media(session: AsyncSession, post: Post) -> None:
    """Riscrive la cache `post_media` a partire dal contenuto del post
    (CLAUDE.md #4) — stessa insidia async di `_sync_post_tags`/`_sync_post_notes`."""
    if post.id is None:
        await session.flush()
    refs = extract_media(post.content)
    await session.execute(delete(post_media).where(post_media.c.post_id == post.id))
    if refs:
        await session.execute(
            insert(post_media),
            [
                {
                    "post_id": post.id,
                    "position": r.position,
                    "url": r.url,
                    "alt_text": r.alt_text,
                    "categories": list(r.categories),
                }
                for r in refs
            ],
        )


async def _sync_post_links(session: AsyncSession, post: Post) -> None:
    """Riscrive la cache `post_links` a partire dal contenuto del post
    (CLAUDE.md #4) — stessa insidia async di `_sync_post_tags`/`_sync_post_notes`."""
    if post.id is None:
        await session.flush()
    refs = extract_links(post.content)
    await session.execute(delete(post_links).where(post_links.c.post_id == post.id))
    if refs:
        await session.execute(
            insert(post_links),
            [{"post_id": post.id, "position": r.position, "url": r.url, "link_text": r.link_text} for r in refs],
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
        notes = normalize_notes(_to_note_inputs(payload.notes))
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    await _validate_category(session, blog, payload.category_id)

    post = Post(
        blog_id=blog.id,
        author_id=current_user.id,
        author_display_name=await _resolve_author_display_name(session, current_user, blog),
        locale=locale,
        title=payload.title,
        slug=payload.slug,
        content=payload.content,
        cover_image_url=payload.cover_image_url,
        cover_image_is_sensitive=payload.cover_image_is_sensitive or bool(payload.cover_image_categories),
        cover_image_categories=payload.cover_image_categories,
        manual_tags=manual_tags,
        category_id=payload.category_id,
    )
    session.add(post)
    await _sync_post_tags(session, post, effective_tags)
    await _sync_post_notes(session, post, notes)
    await _sync_post_media(session, post)
    await _sync_post_links(session, post)
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
        notes = normalize_notes(_to_note_inputs(payload.notes))
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
        author_display_name=await _resolve_author_display_name(session, current_user, blog),
        translation_group_id=original.translation_group_id,
        locale=payload.locale,
        title=payload.title,
        slug=payload.slug,
        content=payload.content,
        cover_image_url=payload.cover_image_url,
        cover_image_is_sensitive=payload.cover_image_is_sensitive or bool(payload.cover_image_categories),
        cover_image_categories=payload.cover_image_categories,
        manual_tags=manual_tags,
        category_id=category_id,
    )
    session.add(translation)
    await _sync_post_tags(session, translation, effective_tags)
    await _sync_post_notes(session, translation, notes)
    await _sync_post_media(session, translation)
    await _sync_post_links(session, translation)
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
    await _require_blog_viewable(session, current_user, blog)

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
    await _require_blog_viewable(session, current_user, blog)
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
    await _require_blog_viewable(session, current_user, blog)
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
        if post.cover_image_url:
            post.cover_image_categories = payload.cover_image_categories or []
            post.cover_image_is_sensitive = bool(payload.cover_image_is_sensitive) or bool(
                post.cover_image_categories
            )
        else:
            # nessuna nuova cover (rimossa): non ha senso restare "sensibile".
            post.cover_image_categories = []
            post.cover_image_is_sensitive = False
    elif payload.cover_image_categories is not None:
        # A differenza del ramo sopra, qui la cover non cambia: il modal di
        # avviso sui contenuti (CLAUDE.md #3) può aggiornare le categorie di
        # un'immagine di copertina già esistente in qualsiasi momento.
        post.cover_image_categories = payload.cover_image_categories
        if post.cover_image_categories:
            post.cover_image_is_sensitive = True

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

    # media e link citati (CLAUDE.md #4): ricalcolati solo se il contenuto è
    # effettivamente cambiato, stesso principio dei tag sopra.
    if payload.content is not None:
        await _sync_post_media(session, post)
        await _sync_post_links(session, post)

    if "category_id" in payload.model_fields_set:
        await _validate_category(session, blog, payload.category_id)
        post.category_id = payload.category_id

    # note: assente lascia invariato; lista (anche `[]`) sostituisce.
    if payload.notes is not None:
        try:
            notes = normalize_notes(_to_note_inputs(payload.notes))
        except ValueError as exc:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        await _sync_post_notes(session, post, notes)

    # todo/USERS.md #2: quando è l'autore stesso a risalvare, riallinea il nome
    # pubblico alle regole correnti (alias imposto dal blog o preferenza di
    # profilo) — non esiste più un valore per singolo post da preservare.
    if current_user.id == post.author_id:
        post.author_display_name = await _resolve_author_display_name(session, current_user, blog)

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
