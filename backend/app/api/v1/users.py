import uuid
from datetime import datetime, timedelta, timezone

from fastapi import APIRouter, Depends, HTTPException, Request, UploadFile, status
from pydantic import BaseModel, EmailStr
from sqlalchemy import any_, delete, func, or_, select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.domain.authorization import blog_publicly_listable_clause
from app.api.v1.blogs._common import BlogOut, _to_blog_out
from app.api.v1.posts import PostOut, _posts_out
from app.core.database import get_session
from app.core.security import hash_password, verify_password
from app.core.storage import avatar_public_url, delete_avatar, upload_avatar
from app.domain import audit
from app.domain import custom_domains as custom_domains_domain
from app.domain import email_change as email_change_domain
from app.domain.fediverse import activitypub_actor_id_for, atproto_did_for
from app.domain.gdpr import anonymize_and_deactivate_user, export_user_data
from app.domain.gdpr_queue import log_self_service_request
from app.models.gdpr_request import GdprRequestType
from app.domain.i18n import validate_locale
from app.domain.interests import validate_user_interest_keys
from app.domain.passwords import validate_password_policy
from app.domain.platform_config import SUPPORTED_LOCALES, get_platform_config
from app.domain.profile import validate_country_code, validate_fallback_languages
from app.domain.rate_limit import enforce_rate_limit
from app.domain.usernames import USERNAME_CHANGE_COOLDOWN_DAYS, validate_username
from app.models.blog import Blog, BlogVisibility
from app.models.comment import Comment, CommentStatus
from app.models.custom_domain import CustomDomain, CustomDomainStatus
from app.models.email_change_request import EmailChangeRequest
from app.models.post import Post, PostStatus
from app.models.follow import BlogFollow, UserFollow
from app.models.social_link import SocialLink
from app.models.user import PostAuthorNameStyle, User, VerificationTier
from app.models.user_session import UserSession

router = APIRouter()


class ProfileUpdateRequest(BaseModel):
    # todo/USERS.md #7: citabile ovunque come @username (menzioni, permalink
    # profilo /u/{username}); cambiarlo si riflette subito ovunque perché
    # tutto il resto del sistema referenzia l'utente per id, non per
    # username — l'unica eccezione nota sono le @menzioni già scritte nel
    # testo dei post/pagine, salvate come testo semplice: restano invariate
    # e puntano allo username precedente. Assente lascia invariato.
    username: str | None = None
    bio: str | None = None
    first_name: str | None = None
    last_name: str | None = None
    # Alias pubblico globale (todo/BLOG.md #4): "" azzera, assente lascia
    # invariato.
    display_name: str | None = None
    # todo/USERS.md #2: come mostrare il proprio nome sui post quando il blog
    # non impone un alias. Assente lascia invariato.
    post_author_name_style: PostAuthorNameStyle | None = None
    # "" per azzerare uno di questi tre; assente lascia invariato (stesso
    # schema di cover_image_url/default_author_display_name altrove)
    country: str | None = None
    native_language: str | None = None
    # assente: lascia invariate; lista (anche vuota) la sostituisce
    fallback_languages: list[str] | None = None
    # B6: lingua dell'interfaccia ("" = torna al default di piattaforma)
    ui_locale: str | None = None
    # Opt-out dalla directory pubblica (GET /users), assente lascia invariato
    directory_listed: bool | None = None
    # Interessi (blocco "interessi utente"): al più 5 chiavi canoniche tra
    # quelle correnti di GET /interests. Assente lascia invariato, una lista
    # (anche vuota) la sostituisce — stesso schema di fallback_languages.
    interests: list[str] | None = None


class SocialLinkCreateRequest(BaseModel):
    label: str
    url: str


class SocialLinkOut(BaseModel):
    id: uuid.UUID
    label: str
    url: str
    position: int

    model_config = {"from_attributes": True}


class AvatarOut(BaseModel):
    avatar_url: str | None


class ProfileOut(BaseModel):
    username: str
    bio: str | None
    first_name: str | None
    last_name: str | None
    display_name: str | None
    post_author_name_style: PostAuthorNameStyle
    country: str | None
    native_language: str | None
    fallback_languages: list[str]
    # Chiavi canoniche (blocco "interessi utente"): il frontend le risolve
    # nella lingua corrente tramite GET /interests, mai stringhe libere.
    interests: list[str]
    avatar_url: str | None
    social_links: list[SocialLinkOut]
    created_at: datetime
    # Sigillo di verifica (CLAUDE.md #5): "none" se mai assegnato.
    verification_tier: VerificationTier
    # Dominio custom, solo se verificato con successo (mai pending/failed) —
    # lo username di piattaforma resta comunque sempre citabile come fallback.
    custom_domain: str | None
    # ID fediverse placeholder (CLAUDE.md #5): calcolati, non federati.
    atproto_did: str
    activitypub_actor_id: str

    model_config = {"from_attributes": True}


class PendingEmailChangeOut(BaseModel):
    new_email: str
    stage: str  # "awaiting_old_confirmation" | "awaiting_new_confirmation"


class MeProfileOut(ProfileOut):
    """Estende `ProfileOut` con i campi privati, mai esposti sul profilo
    pubblico (`GET /{username}`): email, stato del cooldown username, cambio
    email in corso, istruzioni del dominio custom non ancora verificato."""

    email: str
    username_changed_at: datetime | None
    next_username_change_allowed_at: datetime | None
    pending_email_change: PendingEmailChangeOut | None
    domain_pending_verification: str | None
    # `pending`/`failed` del dominio non ancora verificato — senza questo il
    # frontend non ha modo di distinguerli al caricamento del profilo e
    # ricostruiva sempre "pending" a un refresh, perdendo lo stato "failed"
    # persistito in DB (bug segnalato dalla review Copilot).
    domain_status: CustomDomainStatus | None
    domain_verification_instructions: dict | None
    # Impostazione privata (GET /{username} non la espone): opt-out dalla
    # directory pubblica, vedi ProfileUpdateRequest.directory_listed.
    directory_listed: bool


class DomainUpdateRequest(BaseModel):
    domain: str


class DomainOut(BaseModel):
    domain: str
    status: CustomDomainStatus
    txt_record_name: str
    txt_record_value: str


class EmailChangeRequestIn(BaseModel):
    new_email: EmailStr


class EmailChangeCodeIn(BaseModel):
    code: str


class FollowerOut(BaseModel):
    username: str

    model_config = {"from_attributes": True}


class BlogFollowerCountOut(BaseModel):
    blog_slug: str
    blog_title: str
    # Nome sotto cui il blog appare pubblicamente (CLAUDE.md #1): se diverso
    # dallo username di chi lo gestisce, i follower di questo blog non lo
    # associano alla persona reale — vedi FollowStatsOut più sotto.
    alias: str | None
    followers: int


class FollowStatsOut(BaseModel):
    """Solo per il proprietario (`GET /users/me/follow-stats`): l'unico posto
    dove identità reale e alias di blog compaiono fianco a fianco, per
    tenere traccia di quante persone lo seguono in tutto — sotto qualunque
    identità abbia usato per farlo. Il conteggio per singola entità (utente
    reale o blog) resta invece visibile pubblicamente in modo separato,
    tramite gli endpoint /{username}/followers e /blogs/{slug}/followers,
    senza che i due si tocchino mai altrove."""

    user_followers: int
    blogs: list[BlogFollowerCountOut]
    total_followers: int


MAX_SOCIAL_LINKS = 5


async def _find_user_by_username_or_domain(session: AsyncSession, identifier: str) -> User | None:
    """Risolve un utente per username oppure, se non trovato, per dominio
    personalizzato verificato (`User.verified_domain`, copia denormalizzata
    di `CustomDomain.domain` mantenuta in sync solo per lo stato VERIFIED —
    vedi `app/models/user.py`): lo username di piattaforma resta comunque
    sempre citabile/risolvibile (CLAUDE.md #5, ROADMAP.md #1), il dominio è
    un identificativo aggiuntivo e non esclusivo. `identifier` è confrontato
    as-is come username e in lowercase come dominio (i domini sono sempre
    normalizzati in minuscolo, vedi `app/domain/custom_domains.py::validate_domain`)."""
    result = await session.execute(select(User).where(User.username == identifier))
    user = result.scalar_one_or_none()
    if user is not None:
        return user

    result = await session.execute(select(User).where(User.verified_domain == identifier.lower()))
    return result.scalar_one_or_none()


async def _get_user_or_404(session: AsyncSession, username: str) -> User:
    user = await _find_user_by_username_or_domain(session, username)
    if user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    return user


async def _get_user_with_profile_or_404(session: AsyncSession, username: str) -> User:
    user = await _find_user_by_username_or_domain(session, username)
    if user is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Utente non trovato.")
    await session.refresh(user, attribute_names=["social_links"])
    return user


def _to_profile_out(user: User) -> ProfileOut:
    return ProfileOut(
        username=user.username,
        bio=user.bio,
        first_name=user.first_name,
        last_name=user.last_name,
        display_name=user.display_name,
        post_author_name_style=user.post_author_name_style,
        country=user.country,
        native_language=user.native_language,
        fallback_languages=user.fallback_languages,
        interests=user.interests,
        avatar_url=avatar_public_url(user.avatar_object_key) if user.avatar_object_key else None,
        social_links=[SocialLinkOut.model_validate(link) for link in user.social_links],
        created_at=user.created_at,
        verification_tier=user.verification_tier,
        custom_domain=user.verified_domain,
        atproto_did=atproto_did_for(user),
        activitypub_actor_id=activitypub_actor_id_for(user),
    )


async def _to_me_profile_out(session: AsyncSession, user: User) -> MeProfileOut:
    await session.refresh(user, attribute_names=["social_links", "custom_domain"])
    base = _to_profile_out(user)

    pending_result = await session.execute(
        select(EmailChangeRequest)
        .where(EmailChangeRequest.user_id == user.id, EmailChangeRequest.completed_at.is_(None))
        .order_by(EmailChangeRequest.created_at.desc())
    )
    pending = pending_result.scalars().first()
    pending_out = None
    if pending is not None:
        stage = (
            "awaiting_new_confirmation" if pending.old_consumed_at is not None else "awaiting_old_confirmation"
        )
        pending_out = PendingEmailChangeOut(new_email=pending.new_email, stage=stage)

    next_change = None
    if user.username_changed_at is not None:
        next_change = user.username_changed_at + timedelta(days=USERNAME_CHANGE_COOLDOWN_DAYS)

    domain_pending = None
    domain_status = None
    domain_instructions = None
    if user.custom_domain is not None and user.custom_domain.status != CustomDomainStatus.VERIFIED:
        domain_pending = user.custom_domain.domain
        domain_status = user.custom_domain.status
        domain_instructions = {
            "txt_record_name": custom_domains_domain.txt_record_name(user.custom_domain.domain),
            "txt_record_value": custom_domains_domain.txt_record_value(user.custom_domain.verification_token),
        }

    return MeProfileOut(
        **base.model_dump(),
        email=user.email,
        username_changed_at=user.username_changed_at,
        next_username_change_allowed_at=next_change,
        pending_email_change=pending_out,
        domain_pending_verification=domain_pending,
        domain_status=domain_status,
        domain_verification_instructions=domain_instructions,
        directory_listed=user.directory_listed,
    )


class DirectoryUserOut(BaseModel):
    """Voce della directory pubblica (`GET /users`, blocco "directory
    utenti"): sottoinsieme di `ProfileOut` per le card, più il conteggio
    follower — stesso schema di `PublicBlogOut` per la directory blog."""

    username: str
    display_name: str | None
    bio: str | None
    avatar_url: str | None
    verification_tier: VerificationTier
    custom_domain: str | None
    interests: list[str]
    follower_count: int

    model_config = {"from_attributes": True}


@router.get("", response_model=list[DirectoryUserOut])
async def list_public_users(
    q: str | None = None,
    locale: str | None = None,
    interest: str | None = None,
    sort: str = "new",
    limit: int = 30,
    offset: int = 0,
    session: AsyncSession = Depends(get_session),
) -> list[DirectoryUserOut]:
    """Directory pubblica degli utenti: solo account attivi (non anonimizzati/
    cancellati, `is_active`) che non hanno scelto l'opt-out
    (`User.directory_listed`) — stesso principio di `GET /blogs` per i blog,
    ma qui il criterio di esclusione è impostabile dall'utente stesso, non
    derivato da stato dell'account. `q` cerca in username/nome
    pubblico/bio, `locale` filtra per lingua madre, `interest` filtra per
    chiave canonica di interesse (blocco "interessi utente", per trovare
    persone con lo stesso interesse da seguire), `sort` è `new`
    (registrazione, default) o `followers`."""
    limit = max(1, min(limit, 100))
    follower_count = (
        select(func.count())
        .select_from(UserFollow)
        .where(UserFollow.followed_user_id == User.id)
        .correlate(User)
        .scalar_subquery()
    )
    stmt = select(User, follower_count).where(
        User.is_active.is_(True), User.directory_listed.is_(True)
    )
    if q:
        needle = f"%{q.strip()}%"
        stmt = stmt.where(
            or_(User.username.ilike(needle), User.display_name.ilike(needle), User.bio.ilike(needle))
        )
    if locale:
        stmt = stmt.where(User.native_language == locale)
    if interest:
        stmt = stmt.where(interest == any_(User.interests))
    if sort == "followers":
        stmt = stmt.order_by(follower_count.desc(), User.created_at.desc())
    else:
        stmt = stmt.order_by(User.created_at.desc())
    result = await session.execute(stmt.limit(limit).offset(max(offset, 0)))
    return [
        DirectoryUserOut(
            username=user.username,
            display_name=user.display_name,
            bio=user.bio,
            avatar_url=avatar_public_url(user.avatar_object_key) if user.avatar_object_key else None,
            verification_tier=user.verification_tier,
            custom_domain=user.verified_domain,
            interests=user.interests,
            follower_count=int(followers or 0),
        )
        for user, followers in result.all()
    ]


@router.get("/me", response_model=MeProfileOut)
async def get_my_profile(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MeProfileOut:
    """Profilo privato del proprietario (a differenza di `GET /{username}`,
    pubblico): include l'email, che non deve mai comparire sul profilo
    pubblico di nessun utente."""
    return await _to_me_profile_out(session, current_user)


@router.get("/{username}", response_model=ProfileOut)
async def get_profile(username: str, session: AsyncSession = Depends(get_session)) -> ProfileOut:
    user = await _get_user_with_profile_or_404(session, username)
    return _to_profile_out(user)


@router.patch("/me", response_model=MeProfileOut)
async def update_profile(
    payload: ProfileUpdateRequest,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MeProfileOut:
    if payload.username is not None:
        new_username = payload.username.strip().lower()
        if new_username != current_user.username:
            if current_user.username_changed_at is not None:
                cooldown_ends = current_user.username_changed_at + timedelta(
                    days=USERNAME_CHANGE_COOLDOWN_DAYS
                )
                now = datetime.now(timezone.utc)
                if now < cooldown_ends:
                    # data leggibile (non solo ISO): il frontend mostra già in
                    # proattivo next_username_change_allowed_at (GET /users/me)
                    # prima ancora di tentare il salvataggio, qui basta un
                    # messaggio d'errore semplice, stesso stile del resto
                    # dell'API (dettaglio sempre stringa).
                    raise HTTPException(
                        status.HTTP_409_CONFLICT,
                        (
                            f"Puoi cambiare username al massimo una volta ogni "
                            f"{USERNAME_CHANGE_COOLDOWN_DAYS} giorni. Prossimo cambio "
                            f"consentito dal {cooldown_ends.strftime('%d/%m/%Y')}."
                        ),
                    )
            try:
                validate_username(new_username)
            except ValueError as exc:
                raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
            existing = await session.execute(select(User).where(User.username == new_username))
            if existing.scalar_one_or_none() is not None:
                raise HTTPException(status.HTTP_409_CONFLICT, "Username già in uso.")
            old_username = current_user.username
            current_user.username = new_username
            current_user.username_changed_at = datetime.now(timezone.utc)
            await audit.record(
                session,
                action="user.username_changed",
                actor=current_user,
                target_type="user",
                target_id=current_user.id,
                request=request,
                payload={"old_username": old_username, "new_username": new_username},
            )
    if payload.ui_locale is not None:
        if payload.ui_locale and payload.ui_locale not in SUPPORTED_LOCALES:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, "Lingua dell'interfaccia non supportata.")
        current_user.ui_locale = payload.ui_locale or None
    if payload.bio is not None:
        current_user.bio = payload.bio
    if payload.first_name is not None:
        current_user.first_name = payload.first_name or None
    if payload.last_name is not None:
        current_user.last_name = payload.last_name or None
    if payload.display_name is not None:
        current_user.display_name = payload.display_name.strip() or None
    if payload.post_author_name_style is not None:
        current_user.post_author_name_style = payload.post_author_name_style
    if payload.country is not None:
        country = payload.country.strip().upper()
        if country:
            try:
                validate_country_code(country)
            except ValueError as exc:
                raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        current_user.country = country or None
    if payload.native_language is not None:
        native_language = payload.native_language.strip().lower()
        if native_language:
            try:
                validate_locale(native_language)
            except ValueError as exc:
                raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        current_user.native_language = native_language or None
    if payload.fallback_languages is not None:
        normalized = [loc.strip().lower() for loc in payload.fallback_languages]
        try:
            validate_fallback_languages(normalized)
        except ValueError as exc:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
        current_user.fallback_languages = normalized
    if payload.directory_listed is not None:
        current_user.directory_listed = payload.directory_listed
    if payload.interests is not None:
        config = await get_platform_config(session)
        available = {item["key"] for item in config.interests}
        try:
            current_user.interests = validate_user_interest_keys(payload.interests, available)
        except ValueError as exc:
            raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    await session.commit()
    return await _to_me_profile_out(session, current_user)


class PasswordChangeRequest(BaseModel):
    current_password: str
    new_password: str


class StatusOut(BaseModel):
    status: str = "ok"


@router.post("/me/password", response_model=StatusOut)
async def change_my_password(
    payload: PasswordChangeRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> StatusOut:
    """Cambio password autenticato (a differenza del reset "password
    dimenticata" in app/domain/password_reset.py, richiede di conoscere la
    password attuale, non un codice via email)."""
    if current_user.hashed_password is None:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST,
            "Questo account non ha una password impostata (accesso solo tramite SSO).",
        )
    if not verify_password(payload.current_password, current_user.hashed_password):
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Password attuale non corretta.")
    try:
        validate_password_policy(payload.new_password)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    current_user.hashed_password = hash_password(payload.new_password)
    # Come il reset "password dimenticata" (app/domain/password_reset.py):
    # una password cambiata invalida ogni sessione già aperta altrove, non
    # solo il refresh token della richiesta corrente. Le UserSession coprono
    # solo i refresh token; gli access token JWT sono stateless e restano
    # validi fino al loro exp naturale se non si aggiorna anche questo campo
    # (controllato in app/api/deps.py::get_current_user contro l'iat del token).
    current_user.credentials_changed_at = datetime.now(timezone.utc)
    await session.execute(delete(UserSession).where(UserSession.user_id == current_user.id))
    await session.commit()
    return StatusOut()


@router.post("/me/avatar", response_model=AvatarOut)
async def upload_my_avatar(
    file: UploadFile,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> AvatarOut:
    content = await file.read()
    try:
        object_key = upload_avatar(
            user_id=current_user.id, content=content, content_type=file.content_type or ""
        )
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    old_key = current_user.avatar_object_key
    current_user.avatar_object_key = object_key
    await session.commit()

    if old_key is not None:
        delete_avatar(old_key)

    return AvatarOut(avatar_url=avatar_public_url(object_key))


@router.delete("/me/avatar", status_code=status.HTTP_204_NO_CONTENT)
async def delete_my_avatar(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    if current_user.avatar_object_key is not None:
        delete_avatar(current_user.avatar_object_key)
        current_user.avatar_object_key = None
        await session.commit()


@router.post("/me/social-links", response_model=SocialLinkOut, status_code=status.HTTP_201_CREATED)
async def add_social_link(
    payload: SocialLinkCreateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> SocialLink:
    count = await session.execute(
        select(SocialLink).where(SocialLink.user_id == current_user.id)
    )
    existing_links = list(count.scalars().all())
    if len(existing_links) >= MAX_SOCIAL_LINKS:
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST, f"Massimo {MAX_SOCIAL_LINKS} link social per profilo."
        )

    link = SocialLink(
        user_id=current_user.id,
        label=payload.label,
        url=payload.url,
        position=len(existing_links),
    )
    session.add(link)
    await session.commit()
    await session.refresh(link)
    return link


@router.delete("/me/social-links/{link_id}", status_code=status.HTTP_204_NO_CONTENT)
async def delete_social_link(
    link_id: uuid.UUID,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    link = await session.get(SocialLink, link_id)
    if link is None or link.user_id != current_user.id:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Link non trovato.")
    await session.delete(link)
    await session.commit()


@router.post("/me/email/request", status_code=status.HTTP_202_ACCEPTED)
async def request_email_change(
    payload: EmailChangeRequestIn,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict:
    """Passo 1/3 del cambio email: invia un codice alla casella *attuale*
    (prova il possesso dell'account, non solo della sessione)."""
    try:
        await email_change_domain.request_email_change(session, current_user, payload.new_email)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
    return {"detail": "Codice inviato all'indirizzo email attuale."}


@router.post("/me/email/verify-current", status_code=status.HTTP_202_ACCEPTED)
async def verify_current_email(
    payload: EmailChangeCodeIn,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict:
    """Passo 2/3: verifica il codice inviato alla vecchia casella, poi invia
    il codice alla nuova."""
    try:
        await email_change_domain.confirm_old_email(session, current_user, payload.code)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
    return {"detail": "Codice inviato al nuovo indirizzo email."}


@router.delete("/me/email/request", status_code=status.HTTP_204_NO_CONTENT)
async def cancel_email_change(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """Annulla una richiesta di cambio email pending, a qualunque passo si
    trovi (prima o dopo aver verificato la vecchia casella)."""
    await email_change_domain.cancel_email_change(session, current_user)


@router.post("/me/email/verify-new", response_model=MeProfileOut)
async def verify_new_email(
    payload: EmailChangeCodeIn,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> MeProfileOut:
    """Passo 3/3: verifica il codice inviato alla nuova casella e applica il
    cambio."""
    try:
        old_email = await email_change_domain.confirm_new_email(session, current_user, payload.code)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc
    await audit.record(
        session,
        action="user.email_changed",
        actor=current_user,
        target_type="user",
        target_id=current_user.id,
        request=request,
        payload={"old_email": old_email, "new_email": current_user.email},
    )
    await session.commit()
    return await _to_me_profile_out(session, current_user)


@router.post("/me/domain", response_model=DomainOut)
async def set_my_domain(
    payload: DomainUpdateRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> DomainOut:
    """Registra (o sostituisce) il dominio custom in stato `pending`,
    ritornando le istruzioni per il record TXT da pubblicare sul DNS."""
    try:
        normalized = custom_domains_domain.validate_domain(payload.domain)
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    # Solo un claim già VERIFIED di un altro account blocca il dominio (vedi
    # API.md: "409 se già rivendicato e verificato da un altro account") —
    # un pending/failed non prova alcun controllo sul dominio, altrimenti
    # chiunque potrebbe "prenotare" un dominio arbitrario senza mai
    # verificarlo, bloccandolo indefinitamente al vero proprietario.
    existing_owner = await session.execute(
        select(CustomDomain).where(
            CustomDomain.domain == normalized,
            CustomDomain.user_id != current_user.id,
            CustomDomain.status == CustomDomainStatus.VERIFIED,
        )
    )
    if existing_owner.scalar_one_or_none() is not None:
        raise HTTPException(status.HTTP_409_CONFLICT, "Dominio già rivendicato da un altro account.")

    # `CustomDomain.domain` è unique a livello di DB (un solo proprietario per
    # riga, chiunque esso sia): una riga pending/failed di un altro utente non
    # blocca la richiesta sopra, ma resterebbe comunque a occupare la stessa
    # colonna unique e farebbe fallire l'insert/update sotto con un
    # IntegrityError invece che con un 409 pulito. Sotto la stessa regola per
    # cui non blocca ("non prova il possesso"), va rimossa qui: la si
    # considera riconquistabile, non la si "eredita".
    await session.execute(
        delete(CustomDomain).where(
            CustomDomain.domain == normalized,
            CustomDomain.user_id != current_user.id,
            CustomDomain.status != CustomDomainStatus.VERIFIED,
        )
    )

    await session.refresh(current_user, attribute_names=["custom_domain"])
    token = custom_domains_domain.generate_verification_token()
    if current_user.custom_domain is not None:
        record = current_user.custom_domain
        was_verified = record.status == CustomDomainStatus.VERIFIED
        record.domain = normalized
        record.verification_token = token
        record.status = CustomDomainStatus.PENDING
        record.verified_at = None
        # Sostituire un dominio già verificato con uno nuovo (ancora da
        # verificare) non deve lasciare il profilo pubblico a mostrare il
        # vecchio dominio/badge bronzo come se fosse ancora attivo — stessa
        # pulizia di delete_my_domain sotto.
        if was_verified:
            current_user.verified_domain = None
            if current_user.verification_tier == VerificationTier.BRONZE:
                current_user.verification_tier = VerificationTier.NONE
    else:
        record = CustomDomain(
            user_id=current_user.id,
            domain=normalized,
            verification_token=token,
            status=CustomDomainStatus.PENDING,
        )
        session.add(record)
    try:
        await session.commit()
    except IntegrityError as exc:
        # La DELETE sopra rimuove solo le righe pending/failed già viste in
        # questa richiesta: due utenti che rivendicano lo stesso dominio non
        # ancora reclamato in parallelo possono comunque superare entrambi i
        # controlli prima che uno dei due faccia commit — senza questo
        # catch il secondo commit fallisce sul vincolo unique con un 500
        # invece di un 409 pulito (bug segnalato dalla review Copilot).
        await session.rollback()
        raise HTTPException(status.HTTP_409_CONFLICT, "Dominio già rivendicato da un altro account.") from exc
    await session.refresh(record)

    return DomainOut(
        domain=record.domain,
        status=record.status,
        txt_record_name=custom_domains_domain.txt_record_name(record.domain),
        txt_record_value=custom_domains_domain.txt_record_value(record.verification_token),
    )


@router.post("/me/domain/verify", response_model=DomainOut)
async def verify_my_domain(
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> DomainOut:
    """Interroga il DNS e, se il record TXT combacia, verifica il dominio e
    assegna il sigillo di verifica bronzo (mai degrada un tier superiore
    assegnato in futuro da altra logica)."""
    await enforce_rate_limit(
        f"ratelimit:domain-verify:user:{current_user.id}",
        limit=5,
        window_seconds=600,
        message="Troppi tentativi di verifica dominio, riprova più tardi.",
    )
    await session.refresh(current_user, attribute_names=["custom_domain"])
    record = current_user.custom_domain
    if record is None:
        raise HTTPException(status.HTTP_404_NOT_FOUND, "Nessun dominio impostato.")

    verified = await custom_domains_domain.verify_domain_dns(record.domain, record.verification_token)
    if not verified:
        record.status = CustomDomainStatus.FAILED
        await session.commit()
        raise HTTPException(
            status.HTTP_400_BAD_REQUEST,
            "Record TXT non trovato o non corrispondente. Riprova dopo aver aggiornato il DNS.",
        )

    record.status = CustomDomainStatus.VERIFIED
    record.verified_at = datetime.now(timezone.utc)
    if current_user.verification_tier == VerificationTier.NONE:
        current_user.verification_tier = VerificationTier.BRONZE
    current_user.verified_domain = record.domain
    await audit.record(
        session,
        action="user.domain_verified",
        actor=current_user,
        target_type="user",
        target_id=current_user.id,
        request=request,
        payload={"domain": record.domain},
    )
    await session.commit()
    await session.refresh(record)

    return DomainOut(
        domain=record.domain,
        status=record.status,
        txt_record_name=custom_domains_domain.txt_record_name(record.domain),
        txt_record_value=custom_domains_domain.txt_record_value(record.verification_token),
    )


@router.delete("/me/domain", status_code=status.HTTP_204_NO_CONTENT)
async def delete_my_domain(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    await session.refresh(current_user, attribute_names=["custom_domain"])
    record = current_user.custom_domain
    if record is None:
        return
    was_verified = record.status == CustomDomainStatus.VERIFIED
    await session.delete(record)
    if was_verified:
        current_user.verified_domain = None
        if current_user.verification_tier == VerificationTier.BRONZE:
            current_user.verification_tier = VerificationTier.NONE
    await session.commit()


@router.post("/{username}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def follow_user(
    username: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    target = await _get_user_or_404(session, username)
    if target.id == current_user.id:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Non puoi seguire te stesso.")

    existing = await session.execute(
        select(UserFollow).where(
            UserFollow.follower_id == current_user.id, UserFollow.followed_user_id == target.id
        )
    )
    if existing.scalar_one_or_none() is None:
        session.add(UserFollow(follower_id=current_user.id, followed_user_id=target.id))
        await session.commit()


@router.delete("/{username}/follow", status_code=status.HTTP_204_NO_CONTENT)
async def unfollow_user(
    username: str,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    target = await _get_user_or_404(session, username)
    existing = await session.execute(
        select(UserFollow).where(
            UserFollow.follower_id == current_user.id, UserFollow.followed_user_id == target.id
        )
    )
    follow = existing.scalar_one_or_none()
    if follow is not None:
        await session.delete(follow)
        await session.commit()


class PublicCommentOut(BaseModel):
    id: uuid.UUID
    content: str
    created_at: datetime
    post_title: str
    permalink: str


def _signed_as_username(display_name: str, user: User) -> bool:
    return display_name.strip().lower() == user.username.lower()


@router.get("/{username}/blogs", response_model=list[BlogOut])
async def list_user_public_blogs(username: str, session: AsyncSession = Depends(get_session)) -> list[BlogOut]:
    """Blog pubblici di un utente per la tab "Blog" del profilo (mockup 3e).
    CLAUDE.md #8: un blog che si presenta con un alias diverso dallo username
    non viene elencato — altrimenti questo endpoint collegherebbe l'alias
    all'identità reale, cosa che il resto dell'API evita di proposito."""
    user = await _get_user_or_404(session, username)
    result = await session.execute(
        select(Blog)
        .where(Blog.owner_id == user.id, blog_publicly_listable_clause())
        .order_by(Blog.created_at)
    )
    return [
        _to_blog_out(blog, None)
        for blog in result.scalars().all()
        if not blog.default_author_display_name or _signed_as_username(blog.default_author_display_name, user)
    ]


@router.get("/{username}/posts", response_model=list[PostOut])
async def list_user_public_posts(
    username: str, limit: int = 20, offset: int = 0, session: AsyncSession = Depends(get_session)
) -> list[PostOut]:
    """Post pubblicati di un utente su blog pubblici (tab "Post" del profilo).
    Stessa regola di privacy di `/blogs`: solo i post firmati pubblicamente
    con lo username, mai quelli firmati con un alias."""
    user = await _get_user_or_404(session, username)
    limit = max(1, min(limit, 50))
    result = await session.execute(
        select(Post, Blog)
        .join(Blog, Post.blog_id == Blog.id)
        .where(
            Post.author_id == user.id,
            Post.status == PostStatus.PUBLISHED,
            Post.published_at <= datetime.now(timezone.utc),
            Post.is_hidden.is_(False),
            blog_publicly_listable_clause(),
        )
        .order_by(Post.published_at.desc())
        .limit(limit)
        .offset(max(offset, 0))
    )
    pairs = [(post, blog) for post, blog in result.all() if _signed_as_username(post.author_display_name, user)]
    return await _posts_out(session, pairs)


@router.get("/{username}/comments", response_model=list[PublicCommentOut])
async def list_user_public_comments(
    username: str, limit: int = 20, offset: int = 0, session: AsyncSession = Depends(get_session)
) -> list[PublicCommentOut]:
    """Commenti approvati di un utente su post pubblici (tab "Commenti").
    Stessa regola di privacy: solo quelli firmati con lo username."""
    user = await _get_user_or_404(session, username)
    limit = max(1, min(limit, 50))
    result = await session.execute(
        select(Comment, Post.title, Post.slug, Blog.slug)
        .join(Post, Post.id == Comment.post_id)
        .join(Blog, Blog.id == Post.blog_id)
        .where(
            Comment.author_id == user.id,
            Comment.status == CommentStatus.APPROVED,
            Post.status == PostStatus.PUBLISHED,
            Post.is_hidden.is_(False),
            blog_publicly_listable_clause(),
        )
        .order_by(Comment.created_at.desc())
        .limit(limit)
        .offset(max(offset, 0))
    )
    return [
        PublicCommentOut(
            id=comment.id,
            content=comment.content,
            created_at=comment.created_at,
            post_title=post_title,
            permalink=f"/{blog_slug}/{post_slug}",
        )
        for comment, post_title, post_slug, blog_slug in result.all()
        if _signed_as_username(comment.author_display_name, user)
    ]


@router.get("/{username}/followers", response_model=list[FollowerOut])
async def list_followers(username: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    target = await _get_user_or_404(session, username)
    result = await session.execute(
        select(User)
        .join(UserFollow, UserFollow.follower_id == User.id)
        .where(UserFollow.followed_user_id == target.id)
    )
    return list(result.scalars().all())


@router.get("/{username}/following", response_model=list[FollowerOut])
async def list_following(username: str, session: AsyncSession = Depends(get_session)) -> list[User]:
    target = await _get_user_or_404(session, username)
    result = await session.execute(
        select(User)
        .join(UserFollow, UserFollow.followed_user_id == User.id)
        .where(UserFollow.follower_id == target.id)
    )
    return list(result.scalars().all())


@router.get("/me/follow-stats", response_model=FollowStatsOut)
async def my_follow_stats(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> FollowStatsOut:
    """CLAUDE.md #8: quante persone seguono l'utente in tutto, sommando chi lo
    segue con lo username reale (UserFollow) e chi segue uno qualunque dei
    suoi blog (BlogFollow) — anche quando quel blog appare con un alias che
    non rivela chi lo gestisce. Riservato al proprietario: è l'unico posto
    dove le due cose vengono messe insieme."""
    user_followers = await session.scalar(
        select(func.count()).select_from(UserFollow).where(UserFollow.followed_user_id == current_user.id)
    )

    blog_rows = await session.execute(
        select(
            Blog.slug,
            Blog.title,
            Blog.default_author_display_name,
            func.count(BlogFollow.id),
        )
        .outerjoin(BlogFollow, BlogFollow.blog_id == Blog.id)
        .where(Blog.owner_id == current_user.id)
        .group_by(Blog.id)
        .order_by(Blog.title)
    )
    blogs = [
        BlogFollowerCountOut(blog_slug=slug, blog_title=title, alias=alias, followers=count)
        for slug, title, alias, count in blog_rows.all()
    ]

    total = (user_followers or 0) + sum(b.followers for b in blogs)
    return FollowStatsOut(user_followers=user_followers or 0, blogs=blogs, total_followers=total)


class AccountDeleteRequest(BaseModel):
    # richiede di ridigitare il proprio username, non solo un booleano
    # generico "confirm: true": un'azione distruttiva e irreversibile merita
    # una conferma che costringa a un gesto deliberato, coerente con il
    # dialogo di conferma lato dashboard (frontend/src/app/dashboard/profile).
    confirm_username: str


@router.get("/me/export-data")
async def export_my_data(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> dict:
    """Diritto di accesso/portabilità (GDPR Art. 20, ROADMAP.md §1): copia dei
    dati collegati all'account in un unico JSON scaricabile — profilo, blog di
    proprietà, post e commenti scritti (ovunque), frammenti salvati, follow,
    token API (mai i segreti) ed eventi di audit di cui è l'attore."""
    data = await export_user_data(session, current_user)
    await log_self_service_request(session, user=current_user, type_=GdprRequestType.EXPORT)
    return data


@router.delete("/me", status_code=status.HTTP_204_NO_CONTENT)
async def delete_my_account(
    payload: AccountDeleteRequest,
    request: Request,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    """Diritto di cancellazione (GDPR Art. 17, ROADMAP.md §1). Vedi
    app/domain/gdpr.py per cosa viene davvero cancellato e perché blog/post/
    commenti restano (anonimizzati, non un DELETE della riga utente)."""
    if payload.confirm_username != current_user.username:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Username di conferma non corretto.")

    await audit.record(
        session,
        action="user.account_deleted",
        actor=current_user,
        target_type="user",
        target_id=current_user.id,
        request=request,
    )
    await log_self_service_request(session, user=current_user, type_=GdprRequestType.DELETION, commit=False)
    await anonymize_and_deactivate_user(session, current_user)
    await session.commit()
