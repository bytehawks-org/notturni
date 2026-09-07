import secrets
import uuid

from authlib.integrations.starlette_client import OAuthError
from fastapi import APIRouter, Depends, HTTPException, Request, Response, status
from pydantic import BaseModel, EmailStr
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.core.config import settings
from app.core.database import get_session
from app.core.http import client_ip
from app.core.oauth import configured_providers, oauth
from app.core.security import create_mfa_challenge_token, decode_mfa_challenge_token
from app.domain import audit
from app.domain.auth import (
    AuthError,
    authenticate_password,
    issue_session,
    register_user,
    revoke_session,
    rotate_refresh_token,
)
from app.domain.rate_limit import enforce_rate_limit
from app.domain.mfa import (
    generate_totp_secret,
    send_email_otp,
    totp_provisioning_uri,
    totp_qr_code_data_uri,
    verify_email_otp,
    verify_totp_code,
)
from app.domain.sso import ExternalProfile, SsoLinkPending, complete_pending_link, link_or_create_user
from app.models.audit_log import AuditActorType
from app.models.sso_identity import SsoProvider
from app.models.user import MfaMethod, PlatformRole, User

router = APIRouter()

# Protezione brute-force sul login (ROADMAP.md §3, Redis rate limiting): due
# limiti indipendenti, per IP (contiene bot che provano molte email diverse)
# e per email (contiene tentativi mirati su un singolo account anche da IP
# diversi/rotanti). Applicati prima di verificare la password.
LOGIN_IP_RATE_LIMIT = 20
LOGIN_EMAIL_RATE_LIMIT = 5
LOGIN_RATE_LIMIT_WINDOW_SECONDS = 300

# Sessione (ROADMAP.md "Sessione in localStorage"): il refresh token vive
# solo in un cookie httpOnly, mai in JSON/localStorage — l'access token
# (breve durata) resta invece nella risposta, tenuto in memoria dal
# frontend. Path ristretto a /api/v1/auth: il cookie non serve altrove e
# così non viaggia su ogni richiesta all'API. Il cookie CSRF è invece
# leggibile da JS (necessario al pattern double-submit) e su path "/" per
# essere visibile da qualunque pagina del frontend.
REFRESH_COOKIE_NAME = "noct_refresh_token"
REFRESH_COOKIE_PATH = "/api/v1/auth"
CSRF_COOKIE_NAME = "noct_csrf_token"
CSRF_HEADER_NAME = "X-CSRF-Token"


def _set_session_cookies(response: Response, refresh_token: str) -> None:
    max_age = settings.jwt_refresh_token_ttl_days * 24 * 3600
    response.set_cookie(
        REFRESH_COOKIE_NAME,
        refresh_token,
        max_age=max_age,
        httponly=True,
        secure=settings.session_cookie_secure,
        samesite=settings.session_cookie_samesite,
        domain=settings.session_cookie_domain or None,
        path=REFRESH_COOKIE_PATH,
    )
    response.set_cookie(
        CSRF_COOKIE_NAME,
        secrets.token_urlsafe(32),
        max_age=max_age,
        httponly=False,
        secure=settings.session_cookie_secure,
        samesite=settings.session_cookie_samesite,
        domain=settings.session_cookie_domain or None,
        path="/",
    )


def _clear_session_cookies(response: Response) -> None:
    response.delete_cookie(REFRESH_COOKIE_NAME, path=REFRESH_COOKIE_PATH, domain=settings.session_cookie_domain or None)
    response.delete_cookie(CSRF_COOKIE_NAME, path="/", domain=settings.session_cookie_domain or None)


def _clear_refresh_cookie(response: Response) -> None:
    """Solo il cookie di refresh: usata quando un refresh fallisce (token
    scaduto/già rotato) — il cookie CSRF non ha valore di confidenzialità e
    lasciarlo in vita evita di rompere una richiesta legittima concorrente."""
    response.delete_cookie(REFRESH_COOKIE_NAME, path=REFRESH_COOKIE_PATH, domain=settings.session_cookie_domain or None)


def _verify_csrf(request: Request) -> None:
    """Double-submit cookie: richiesto sui soli endpoint autenticati dal
    cookie di refresh (refresh/logout) — tutte le altre scritture dell'API
    passano l'access token via header Authorization, già di per sé immune a
    CSRF (un'origine estranea non può impostarlo su una richiesta cross-site)."""
    cookie_token = request.cookies.get(CSRF_COOKIE_NAME)
    header_token = request.headers.get(CSRF_HEADER_NAME)
    if not cookie_token or not header_token or not secrets.compare_digest(cookie_token, header_token):
        raise HTTPException(status.HTTP_403_FORBIDDEN, "CSRF token mancante o non valido.")


# ---- schemi ----------------------------------------------------------------


class RegisterRequest(BaseModel):
    username: str
    email: EmailStr
    password: str


class UserOut(BaseModel):
    id: uuid.UUID
    username: str
    email: str
    display_name: str | None = None
    mfa_enabled: bool
    platform_role: PlatformRole

    model_config = {"from_attributes": True}


class LoginRequest(BaseModel):
    email: EmailStr
    password: str


class SessionResponse(BaseModel):
    access_token: str
    token_type: str = "bearer"


class MfaRequiredResponse(BaseModel):
    mfa_required: bool = True
    method: str
    challenge: str


class MfaVerifyRequest(BaseModel):
    challenge: str
    code: str


class TotpSetupResponse(BaseModel):
    secret: str
    provisioning_uri: str
    # SVG inline come data URI (mai generato da un servizio di terze parti:
    # il secret non deve lasciare il backend) — vedi app/domain/mfa.py.
    qr_code_data_uri: str


class MfaCodeRequest(BaseModel):
    code: str


# ---- registrazione / login --------------------------------------------------


@router.post("/register", response_model=UserOut, status_code=status.HTTP_201_CREATED)
async def register(payload: RegisterRequest, session: AsyncSession = Depends(get_session)) -> User:
    try:
        return await register_user(
            session, username=payload.username, email=payload.email, password=payload.password
        )
    except AuthError as exc:
        raise HTTPException(status.HTTP_409_CONFLICT, str(exc)) from exc
    except ValueError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc


@router.post("/login", response_model=SessionResponse | MfaRequiredResponse)
async def login(
    payload: LoginRequest, request: Request, response: Response, session: AsyncSession = Depends(get_session)
):
    ip = client_ip(request)
    if ip is not None:
        await enforce_rate_limit(
            f"ratelimit:login:ip:{ip}",
            limit=LOGIN_IP_RATE_LIMIT,
            window_seconds=LOGIN_RATE_LIMIT_WINDOW_SECONDS,
            message="Troppi tentativi di accesso da questo indirizzo. Riprova tra qualche minuto.",
        )
    await enforce_rate_limit(
        f"ratelimit:login:email:{payload.email.lower()}",
        limit=LOGIN_EMAIL_RATE_LIMIT,
        window_seconds=LOGIN_RATE_LIMIT_WINDOW_SECONDS,
        message="Troppi tentativi di accesso per questo account. Riprova tra qualche minuto.",
    )

    try:
        user = await authenticate_password(session, email=payload.email, password=payload.password)
    except AuthError as exc:
        await audit.record(
            session,
            action="auth.login_failed",
            actor_type=AuditActorType.ANONYMOUS,
            actor_label=payload.email,
            request=request,
            payload={"email": payload.email, "reason": str(exc)},
        )
        await session.commit()
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, str(exc)) from exc

    if not user.mfa_enabled or user.mfa_method is None:
        await audit.record(
            session, action="auth.login", actor=user, request=request, payload={"method": "password"}
        )
        access_token, refresh_token = await issue_session(session, user)
        _set_session_cookies(response, refresh_token)
        return SessionResponse(access_token=access_token)

    if user.mfa_method == MfaMethod.EMAIL:
        await send_email_otp(session, user)

    challenge = create_mfa_challenge_token(user.id, user.mfa_method.value)
    return MfaRequiredResponse(method=user.mfa_method.value, challenge=challenge)


@router.post("/mfa/verify", response_model=SessionResponse)
async def verify_mfa(
    payload: MfaVerifyRequest,
    request: Request,
    response: Response,
    session: AsyncSession = Depends(get_session),
) -> SessionResponse:
    try:
        claims = decode_mfa_challenge_token(payload.challenge)
    except ValueError as exc:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, str(exc)) from exc

    user = await session.get(User, uuid.UUID(claims["sub"]))
    if user is None or not user.is_active:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "Utente non valido.")

    method = claims["method"]
    ok = (
        verify_totp_code(user.mfa_totp_secret, payload.code)
        if method == "totp"
        else await verify_email_otp(session, user, payload.code)
    )
    if not ok:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "Codice MFA non valido o scaduto.")

    pending_link = claims.get("pending_sso_link")
    if pending_link is not None:
        profile = ExternalProfile(
            provider=SsoProvider(pending_link["provider"]),
            provider_user_id=pending_link["provider_user_id"],
            email=pending_link["email"],
        )
        user = await complete_pending_link(session, user, profile)

    await audit.record(
        session, action="auth.login", actor=user, request=request, payload={"method": f"mfa_{method}"}
    )
    access_token, refresh_token = await issue_session(session, user)
    _set_session_cookies(response, refresh_token)
    return SessionResponse(access_token=access_token)


@router.post("/refresh", response_model=SessionResponse)
async def refresh(request: Request, response: Response, session: AsyncSession = Depends(get_session)) -> SessionResponse:
    _verify_csrf(request)
    refresh_token = request.cookies.get(REFRESH_COOKIE_NAME)
    if not refresh_token:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, "Sessione assente.")
    try:
        access_token, new_refresh_token = await rotate_refresh_token(session, refresh_token)
    except AuthError as exc:
        _clear_refresh_cookie(response)
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, str(exc)) from exc
    _set_session_cookies(response, new_refresh_token)
    return SessionResponse(access_token=access_token)


@router.post("/logout", status_code=status.HTTP_204_NO_CONTENT)
async def logout(request: Request, response: Response, session: AsyncSession = Depends(get_session)) -> None:
    _verify_csrf(request)
    refresh_token = request.cookies.get(REFRESH_COOKIE_NAME)
    if refresh_token:
        await revoke_session(session, refresh_token)
    _clear_session_cookies(response)


@router.get("/me", response_model=UserOut)
async def me(current_user: User = Depends(get_current_user)) -> User:
    return current_user


# ---- MFA: gestione (richiede sessione attiva) -------------------------------


@router.post("/mfa/totp/setup", response_model=TotpSetupResponse)
async def setup_totp(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> TotpSetupResponse:
    """Genera un nuovo secret TOTP (non ancora attivo: serve confermarlo con
    /mfa/totp/confirm)."""
    secret = generate_totp_secret()
    current_user.mfa_totp_secret = secret
    await session.commit()
    provisioning_uri = totp_provisioning_uri(secret, current_user.email)
    return TotpSetupResponse(
        secret=secret,
        provisioning_uri=provisioning_uri,
        qr_code_data_uri=totp_qr_code_data_uri(provisioning_uri),
    )


@router.post("/mfa/totp/confirm", status_code=status.HTTP_204_NO_CONTENT)
async def confirm_totp(
    payload: MfaCodeRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    if current_user.mfa_totp_secret is None or not verify_totp_code(
        current_user.mfa_totp_secret, payload.code
    ):
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Codice non valido.")
    current_user.mfa_enabled = True
    current_user.mfa_method = MfaMethod.TOTP
    await session.commit()


@router.post("/mfa/email/setup", status_code=status.HTTP_202_ACCEPTED)
async def setup_email_mfa(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    await send_email_otp(session, current_user)


@router.post("/mfa/email/confirm", status_code=status.HTTP_204_NO_CONTENT)
async def confirm_email_mfa(
    payload: MfaCodeRequest,
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    if not await verify_email_otp(session, current_user, payload.code):
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Codice non valido o scaduto.")
    current_user.mfa_enabled = True
    current_user.mfa_method = MfaMethod.EMAIL
    await session.commit()


@router.post("/mfa/disable", status_code=status.HTTP_204_NO_CONTENT)
async def disable_mfa(
    current_user: User = Depends(get_current_user),
    session: AsyncSession = Depends(get_session),
) -> None:
    current_user.mfa_enabled = False
    current_user.mfa_method = None
    current_user.mfa_totp_secret = None
    await session.commit()


# ---- SSO ---------------------------------------------------------------


@router.get("/sso/{provider}/login")
async def sso_login(provider: str, request: Request):
    if provider not in configured_providers():
        raise HTTPException(status.HTTP_503_SERVICE_UNAVAILABLE, f"Provider '{provider}' non configurato.")
    redirect_uri = f"{settings.oauth_redirect_base_url}/api/v1/auth/sso/{provider}/callback"
    client = oauth.create_client(provider)
    return await client.authorize_redirect(request, redirect_uri)


@router.get("/sso/{provider}/callback", response_model=SessionResponse | MfaRequiredResponse)
async def sso_callback(
    provider: str, request: Request, response: Response, session: AsyncSession = Depends(get_session)
):
    if provider not in configured_providers():
        raise HTTPException(status.HTTP_503_SERVICE_UNAVAILABLE, f"Provider '{provider}' non configurato.")

    client = oauth.create_client(provider)
    try:
        token = await client.authorize_access_token(request)
    except OAuthError as exc:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, str(exc)) from exc

    if provider == "github":
        profile_data = (await client.get("user", token=token)).json()
        provider_user_id = str(profile_data["id"])
        email = profile_data.get("email")
        if not email:
            emails = (await client.get("user/emails", token=token)).json()
            primary = next((e for e in emails if e.get("primary")), None)
            email = primary["email"] if primary else None
    else:
        profile_data = token.get("userinfo") or await client.userinfo(token=token)
        provider_user_id = profile_data["sub"]
        email = profile_data.get("email")

    if not email:
        raise HTTPException(status.HTTP_400_BAD_REQUEST, "Email non disponibile dal provider.")

    ext_profile = ExternalProfile(
        provider=SsoProvider(provider), provider_user_id=provider_user_id, email=email
    )
    try:
        user = await link_or_create_user(session, ext_profile)
    except SsoLinkPending as pending:
        assert pending.user.mfa_method is not None
        challenge = create_mfa_challenge_token(
            pending.user.id,
            pending.user.mfa_method.value,
            pending_sso_link={
                "provider": pending.profile.provider.value,
                "provider_user_id": pending.profile.provider_user_id,
                "email": pending.profile.email,
            },
        )
        if pending.user.mfa_method == MfaMethod.EMAIL:
            await send_email_otp(session, pending.user)
        return MfaRequiredResponse(method=pending.user.mfa_method.value, challenge=challenge)

    await audit.record(
        session, action="auth.login", actor=user, request=request, payload={"method": f"sso_{provider}"}
    )
    access_token, refresh_token = await issue_session(session, user)
    _set_session_cookies(response, refresh_token)
    return SessionResponse(access_token=access_token)
