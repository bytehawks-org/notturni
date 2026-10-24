import uuid
from datetime import datetime, timezone

from fastapi import APIRouter, Depends, HTTPException, status
from pydantic import BaseModel
from sqlalchemy import func, select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.api.v1.posts import PostOut, _posts_out
from app.core.database import get_session
from app.domain.authorization import can_view_blog, is_publicly_visible
from app.domain.blog_scope import my_blog_ids
from app.domain.fragments import validate_fragment_text
from app.domain.permalinks import build_permalink
from app.models.blog import Blog
from app.models.blog_note import BlogNote
from app.models.media_file import MediaFile
from app.models.post import Post, PostStatus
from app.models.post_fragment import PostFragment
from app.models.post_link import post_links
from app.models.post_media import post_media
from app.models.post_note import post_notes
from app.models.publication import Publication
from app.models.user import User

router = APIRouter()


class FragmentCreateRequest(BaseModel):
    text: str
    # Scelto al salvataggio (ROADMAP.md §1): default privato. "Pubblico" =
    # visibile ad altri utenti iscritti alla piattaforma, mai a visitatori
    # anonimi. Modificabile anche dopo, vedi PATCH sotto.
    is_public: bool = False


class FragmentUpdateRequest(BaseModel):
    is_public: bool


class FragmentOut(BaseModel):
    id: uuid.UUID
    post_id: uuid.UUID
    text: str
    is_public: bool
    created_at: datetime

    model_config = {"from_attributes": True}


class FragmentCollectionOut(BaseModel):
    id: uuid.UUID
    text: str
    is_public: bool
    created_at: datetime
    post_title: str
    author_display_name: str
    permalink: str


async def _get_post_and_blog_or_404(session: AsyncSession, post_id: uuid.UUID) -> tuple[Post, Blog]:
    post = await session.get(Post, post_id)
    if post is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")
    blog = await session.get(Blog, post.blog_id)
    assert blog is not None
    return post, blog


async def _require_fragmentable(session: AsyncSession, current_user: User, post: Post, blog: Blog) -> None:
    """Un frammento si salva solo su un post pubblicato e visibile all'utente
    corrente (stessa regola d'accesso del permalink pubblico) — mai su una
    bozza, che può ancora cambiare da un momento all'altro."""
    if not is_publicly_visible(post) or not await can_view_blog(session, user_id=current_user.id, blog=blog):
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Post non trovato.")


@router.post("/posts/{post_id}/fragments", response_model=FragmentOut, status_code=status.HTTP_201_CREATED)
async def create_fragment(
    post_id: uuid.UUID,
    payload: FragmentCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> FragmentOut:
    post, blog = await _get_post_and_blog_or_404(session, post_id)
    await _require_fragmentable(session, current_user, post, blog)

    try:
        text = validate_fragment_text(payload.text, post.content)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    async def _existing() -> PostFragment | None:
        result = await session.execute(
            select(PostFragment).where(
                PostFragment.user_id == current_user.id,
                PostFragment.post_id == post_id,
                PostFragment.text == text,
            )
        )
        return result.scalar_one_or_none()

    fragment = await _existing()
    if fragment is not None:
        # stesso frammento già salvato: idempotente, non un errore (evita che
        # un doppio click sul menu contestuale del frontend produca un 409).
        return FragmentOut.model_validate(fragment)

    fragment = PostFragment(user_id=current_user.id, post_id=post_id, text=text, is_public=payload.is_public)
    session.add(fragment)
    try:
        await session.commit()
    except IntegrityError:
        # corsa tra due richieste identiche in parallelo: la seconda trova il
        # vincolo unique(user, post, text) già violato dalla prima.
        await session.rollback()
        fragment = await _existing()
        assert fragment is not None
        return FragmentOut.model_validate(fragment)
    await session.refresh(fragment)
    return FragmentOut.model_validate(fragment)


@router.get("/posts/{post_id}/fragments", response_model=list[FragmentOut])
async def list_post_fragments(
    post_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[FragmentOut]:
    """Frammenti già salvati dall'utente corrente su questo post — il
    frontend li usa per ri-evidenziarli ad ogni lettura, indipendentemente
    dal fatto che si sia arrivati al post dalla pagina di raccolta."""
    result = await session.execute(
        select(PostFragment).where(PostFragment.user_id == current_user.id, PostFragment.post_id == post_id)
    )
    return [FragmentOut.model_validate(f) for f in result.scalars().all()]


@router.get("/users/me/fragments", response_model=list[FragmentCollectionOut])
async def list_my_fragments(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[FragmentCollectionOut]:
    """Raccolta unificata dei frammenti salvati dall'utente, più recenti prima."""
    result = await session.execute(
        select(PostFragment, Post, Blog)
        .join(Post, Post.id == PostFragment.post_id)
        .join(Blog, Blog.id == Post.blog_id)
        .where(PostFragment.user_id == current_user.id)
        .order_by(PostFragment.created_at.desc())
    )
    return [
        FragmentCollectionOut(
            id=fragment.id,
            text=fragment.text,
            is_public=fragment.is_public,
            created_at=fragment.created_at,
            post_title=post.title,
            # Colonna già risolta al salvataggio/ultima modifica del post
            # (Post.author_display_name), non ricalcolata da un alias
            # cambiato dopo come fa invece PostOut: semplificazione accettata
            # per una vista derivata/secondaria, non canonica come il post stesso.
            author_display_name=post.author_display_name,
            permalink=build_permalink(blog.slug, post),
        )
        for fragment, post, blog in result.all()
    ]


@router.patch("/fragments/{fragment_id}", response_model=FragmentOut)
async def update_fragment(
    fragment_id: uuid.UUID,
    payload: FragmentUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> FragmentOut:
    """Cambia la visibilità di un frammento già salvato (ROADMAP.md §1: la
    scelta al salvataggio è anche modificabile ex-post, dalla raccolta)."""
    fragment = await session.get(PostFragment, fragment_id)
    if fragment is None or fragment.user_id != current_user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Frammento non trovato.")
    fragment.is_public = payload.is_public
    await session.commit()
    await session.refresh(fragment)
    return FragmentOut.model_validate(fragment)


@router.delete("/fragments/{fragment_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_fragment(
    fragment_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    fragment = await session.get(PostFragment, fragment_id)
    if fragment is None or fragment.user_id != current_user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Frammento non trovato.")
    await session.delete(fragment)
    await session.commit()


# ---------------------------------------------------------------------------
# Viste aggregate "tutti i miei blog" (todo/UX_REDESIGN.md): stessa forma dei
# corrispondenti endpoint per-blog (posts.py, blogs/media.py,
# blogs/publications.py, blogs/bibliography.py), ma su tutti i blog di
# proprietà o membership dell'utente insieme — non richiedono uno slug, non
# sono paginati (come i loro equivalenti per-blog), sempre autore-facing
# (mai filtrati dalla visibilità pubblica: un autore vede anche le proprie
# bozze/post privati qui, a differenza delle bibliografie pubbliche).
# ---------------------------------------------------------------------------


@router.get("/users/me/posts", response_model=list[PostOut])
async def list_my_posts(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[PostOut]:
    """Tutti i post (qualunque stato: bozza/revisione/pubblicato/pianificato)
    di tutti i blog di cui l'utente è proprietario o collaboratore, dal più
    recente. Vista autore, non pubblica: generalizza `GET
    /blogs/{slug}/posts` a più blog invece di uno solo."""
    blog_ids = await my_blog_ids(session, current_user.id)
    if not blog_ids:
        return []
    result = await session.execute(
        select(Post, Blog)
        .join(Blog, Post.blog_id == Blog.id)
        .where(Post.blog_id.in_(blog_ids))
        .order_by(Post.created_at.desc())
    )
    return await _posts_out(session, [(post, blog) for post, blog in result.all()])


class MyMediaUsageOut(BaseModel):
    post_id: uuid.UUID
    post_slug: str
    post_title: str
    permalink: str


class MyMediaFileOut(BaseModel):
    id: uuid.UUID
    url: str
    content_type: str
    size_bytes: int
    alt_text: str
    caption: str | None
    categories: list[str]
    is_sensitive: bool
    uploader_username: str | None
    created_at: datetime
    used_in: list[MyMediaUsageOut]
    blog_slug: str
    blog_title: str


@router.get("/users/me/media", response_model=list[MyMediaFileOut])
async def list_my_media(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MyMediaFileOut]:
    """Libreria media di tutti i blog dell'utente insieme (generalizza `GET
    /blogs/{slug}/media`, vedi app/api/v1/blogs/media.py), con l'attribuzione
    del blog di appartenenza per ogni riga."""
    blog_ids = await my_blog_ids(session, current_user.id)
    if not blog_ids:
        return []

    rows = await session.execute(
        select(MediaFile, Blog)
        .join(Blog, Blog.id == MediaFile.blog_id)
        .where(MediaFile.blog_id.in_(blog_ids))
        .order_by(MediaFile.created_at.desc())
    )
    pairs = [(media, blog) for media, blog in rows.all()]

    urls = [media.url for media, _ in pairs]
    usages: dict[str, list[MyMediaUsageOut]] = {}
    if urls:
        usage_rows = (
            await session.execute(
                select(post_media.c.url, Post.id, Post.slug, Post.title, Blog.slug)
                .join(Post, Post.id == post_media.c.post_id)
                .join(Blog, Blog.id == Post.blog_id)
                .where(Post.blog_id.in_(blog_ids), post_media.c.url.in_(urls))
            )
        ).all()
        seen: set[tuple[str, uuid.UUID]] = set()
        for url, post_id, post_slug, post_title, blog_slug in usage_rows:
            if (url, post_id) in seen:
                continue
            seen.add((url, post_id))
            usages.setdefault(url, []).append(
                MyMediaUsageOut(
                    post_id=post_id, post_slug=post_slug, post_title=post_title, permalink=f"/{blog_slug}/{post_slug}"
                )
            )

    uploader_ids = {media.uploader_id for media, _ in pairs if media.uploader_id}
    names: dict[uuid.UUID, str] = {}
    if uploader_ids:
        names = dict((await session.execute(select(User.id, User.username).where(User.id.in_(uploader_ids)))).all())

    return [
        MyMediaFileOut(
            id=media.id,
            url=media.url,
            content_type=media.content_type,
            size_bytes=media.size_bytes,
            alt_text=media.alt_text,
            caption=media.caption,
            categories=list(media.categories or []),
            is_sensitive=media.is_sensitive,
            uploader_username=names.get(media.uploader_id) if media.uploader_id else None,
            created_at=media.created_at,
            used_in=usages.get(media.url, []),
            blog_slug=blog.slug,
            blog_title=blog.title,
        )
        for media, blog in pairs
    ]


class MyPublicationOut(BaseModel):
    id: uuid.UUID
    name: str
    title: str
    description: str | None
    chapters_total: int
    chapters_published: int
    created_at: datetime
    blog_slug: str
    blog_title: str


@router.get("/users/me/publications", response_model=list[MyPublicationOut])
async def list_my_publications(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MyPublicationOut]:
    """Tutte le pubblicazioni dei blog dell'utente insieme (generalizza `GET
    /blogs/{slug}/publications`, vedi app/api/v1/blogs/publications.py), con
    i conteggi dei capitoli invariati e l'attribuzione del blog per riga."""
    blog_ids = await my_blog_ids(session, current_user.id)
    if not blog_ids:
        return []

    rows = await session.execute(
        select(Publication, Blog)
        .join(Blog, Blog.id == Publication.blog_id)
        .where(Publication.blog_id.in_(blog_ids))
        .order_by(Publication.created_at.desc())
    )
    pairs = [(pub, blog) for pub, blog in rows.all()]
    pub_ids = [pub.id for pub, _ in pairs]

    total: dict[uuid.UUID, int] = {}
    published: dict[uuid.UUID, int] = {}
    if pub_ids:
        total = dict(
            (
                await session.execute(
                    select(Post.publication_id, func.count())
                    .where(Post.publication_id.in_(pub_ids))
                    .group_by(Post.publication_id)
                )
            ).all()
        )
        published = dict(
            (
                await session.execute(
                    select(Post.publication_id, func.count())
                    .where(
                        Post.publication_id.in_(pub_ids),
                        Post.status == PostStatus.PUBLISHED,
                        Post.published_at <= datetime.now(timezone.utc),
                        Post.is_hidden.is_(False),
                    )
                    .group_by(Post.publication_id)
                )
            ).all()
        )

    return [
        MyPublicationOut(
            id=pub.id,
            name=pub.name,
            title=pub.title,
            description=pub.description,
            chapters_total=int(total.get(pub.id, 0)),
            chapters_published=int(published.get(pub.id, 0)),
            created_at=pub.created_at,
            blog_slug=blog.slug,
            blog_title=blog.title,
        )
        for pub, blog in pairs
    ]


class MyContentCitationOut(BaseModel):
    post_title: str
    post_slug: str
    permalink: str
    locale: str
    used_at: datetime | None
    blog_slug: str
    blog_title: str


class MyLinkBibliographyEntryOut(BaseModel):
    url: str
    link_text: str
    citations: list[MyContentCitationOut]


@router.get("/users/me/links", response_model=list[MyLinkBibliographyEntryOut])
async def list_my_links(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MyLinkBibliographyEntryOut]:
    """Link citati nel corpo di tutti i post (qualunque stato) di tutti i
    blog dell'utente, raggruppati per URL — generalizza `GET
    /blogs/{slug}/links-bibliography` (vedi app/api/v1/blogs/bibliography.py)
    a più blog. Vista autore: a differenza dell'originale pubblico, non
    applica `publicly_visible_clause()`, un autore vede qui anche i link
    delle proprie bozze/post privati."""
    blog_ids = await my_blog_ids(session, current_user.id)
    if not blog_ids:
        return []

    rows = await session.execute(
        select(Post, Blog, post_links.c.link_text, post_links.c.url)
        .join(Blog, Blog.id == Post.blog_id)
        .join(post_links, post_links.c.post_id == Post.id)
        .where(Post.blog_id.in_(blog_ids))
        .order_by(Post.created_at.desc(), post_links.c.position.asc())
    )

    entries: dict[str, MyLinkBibliographyEntryOut] = {}
    for post, blog, link_text, url in rows.all():
        entry = entries.get(url)
        if entry is None:
            entry = MyLinkBibliographyEntryOut(url=url, link_text=link_text, citations=[])
            entries[url] = entry
        entry.citations.append(
            MyContentCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                used_at=post.published_at,
                blog_slug=blog.slug,
                blog_title=blog.title,
            )
        )
    return list(entries.values())


class MyBibliographyCitationOut(BaseModel):
    post_title: str
    post_slug: str
    permalink: str
    locale: str
    idx: int
    blog_slug: str
    blog_title: str


class MyBibliographyEntryOut(BaseModel):
    content: str
    kind: str | None = None
    url: str | None = None
    title: str | None = None
    author: str | None = None
    isbn: str | None = None
    doi: str | None = None
    page: str | None = None
    source: str | None = None
    issued: str | None = None
    citations: list[MyBibliographyCitationOut]


def _safe_note_url(url: str | None) -> str | None:
    """Stessa cautela di app/api/v1/blogs/bibliography.py: righe `BlogNote.url`
    create prima della validazione di schema possono ancora contenere un
    valore non http(s), va filtrato qui in lettura."""
    if url is None or url.startswith("http://") or url.startswith("https://"):
        return url
    return None


@router.get("/users/me/bibliography", response_model=list[MyBibliographyEntryOut])
async def list_my_bibliography(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> list[MyBibliographyEntryOut]:
    """Libreria personale delle note a piè di pagina di tutti i post
    (qualunque stato) di tutti i blog dell'utente, raggruppate per testo
    identico — generalizza `GET /blogs/{slug}/bibliography` (vedi
    app/api/v1/blogs/bibliography.py) a più blog insieme, come una
    "raccolta" nello spirito di `GET /users/me/fragments`. Vista autore: non
    applica `publicly_visible_clause()`, mostra anche le note di bozze/post
    privati."""
    blog_ids = await my_blog_ids(session, current_user.id)
    if not blog_ids:
        return []

    rows = await session.execute(
        select(
            Post,
            Blog,
            post_notes.c.idx,
            post_notes.c.content,
            post_notes.c.title,
            post_notes.c.author,
            post_notes.c.isbn,
            post_notes.c.doi,
            post_notes.c.page,
            post_notes.c.source,
            post_notes.c.issued,
            BlogNote.kind,
            BlogNote.url,
        )
        .join(Blog, Blog.id == Post.blog_id)
        .join(post_notes, post_notes.c.post_id == Post.id)
        .outerjoin(BlogNote, BlogNote.id == post_notes.c.note_id)
        .where(Post.blog_id.in_(blog_ids))
        .order_by(Post.created_at.desc(), post_notes.c.idx.asc())
    )

    entries: dict[str, MyBibliographyEntryOut] = {}
    for post, blog, idx, content, title, author, isbn, doi, page, source, issued, kind, url in rows.all():
        key = " ".join(content.split()).casefold()
        entry = entries.get(key)
        if entry is None:
            entry = MyBibliographyEntryOut(
                content=content,
                kind=kind,
                url=_safe_note_url(url),
                title=title,
                author=author,
                isbn=isbn,
                doi=doi,
                page=page,
                source=source,
                issued=issued,
                citations=[],
            )
            entries[key] = entry
        entry.citations.append(
            MyBibliographyCitationOut(
                post_title=post.title,
                post_slug=post.slug,
                permalink=build_permalink(blog.slug, post),
                locale=post.locale,
                idx=idx,
                blog_slug=blog.slug,
                blog_title=blog.title,
            )
        )
    return list(entries.values())
