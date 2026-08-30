import uuid

from authlib.integrations.starlette_client import OAuthError
from fastapi import APIRouter, Depends, HTTPException, Request, status
from pydantic import BaseModel, EmailStr
from sqlalchemy.ext.asyncio import AsyncSession

from app.api.deps import get_current_user
from app.core.config import settings
from app.core.database import get_session
from app.core.oauth import configured_providers, oauth
from app.core.security import create_mfa_challenge_token, decode_mfa_challenge_token
from app.domain.auth import (
    AuthError,
    authenticate_password,
    issue_session,
    register_user,
    revoke_session,
    rotate_refresh_token,
)
from app.domain.mfa import (
    generate_totp_secret,
    send_email_otp,
    totp_provisioning_uri,
    verify_email_otp,
    verify_totp_code,
)
from app.domain.sso import ExternalProfile, SsoLinkPending, complete_pending_link, link_or_create_user
from app.models.sso_identity import SsoProvider
from app.models.user import MfaMethod, PlatformRole, User

router = APIRouter()


# ---- schemi ----------------------------------------------------------------


class RegisterRequest(BaseModel):
    username: str
    email: EmailStr
    password: str


class UserOut(BaseModel):
    id: uuid.UUID
    username: str
    email: str
    mfa_enabled: bool
    platform_role: PlatformRole

    model_config = {"from_attributes": True}


class LoginRequest(BaseModel):
    email: EmailStr
    password: str


class SessionResponse(BaseModel):
    access_token: str
    refresh_token: str
    token_type: str = "bearer"


class MfaRequiredResponse(BaseModel):
    mfa_required: bool = True
    method: str
    challenge: str


class MfaVerifyRequest(BaseModel):
    challenge: str
    code: str


class RefreshRequest(BaseModel):
    refresh_token: str


class LogoutRequest(BaseModel):
    refresh_token: str


class TotpSetupResponse(BaseModel):
    secret: str
    provisioning_uri: str


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
async def login(payload: LoginRequest, session: AsyncSession = Depends(get_session)):
    try:
        user = await authenticate_password(session, email=payload.email, password=payload.password)
    except AuthError as exc:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, str(exc)) from exc

    if not user.mfa_enabled or user.mfa_method is None:
        access_token, refresh_token = await issue_session(session, user)
        return SessionResponse(access_token=access_token, refresh_token=refresh_token)

    if user.mfa_method == MfaMethod.EMAIL:
        await send_email_otp(session, user)

    challenge = create_mfa_challenge_token(user.id, user.mfa_method.value)
    return MfaRequiredResponse(method=user.mfa_method.value, challenge=challenge)


@router.post("/mfa/verify", response_model=SessionResponse)
async def verify_mfa(payload: MfaVerifyRequest, session: AsyncSession = Depends(get_session)) -> SessionResponse:
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

    access_token, refresh_token = await issue_session(session, user)
    return SessionResponse(access_token=access_token, refresh_token=refresh_token)


@router.post("/refresh", response_model=SessionResponse)
async def refresh(payload: RefreshRequest, session: AsyncSession = Depends(get_session)) -> SessionResponse:
    try:
        access_token, refresh_token = await rotate_refresh_token(session, payload.refresh_token)
    except AuthError as exc:
        raise HTTPException(status.HTTP_401_UNAUTHORIZED, str(exc)) from exc
    return SessionResponse(access_token=access_token, refresh_token=refresh_token)


@router.post("/logout", status_code=status.HTTP_204_NO_CONTENT)
async def logout(payload: LogoutRequest, session: AsyncSession = Depends(get_session)) -> None:
    await revoke_session(session, payload.refresh_token)


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
    return TotpSetupResponse(
        secret=secret, provisioning_uri=totp_provisioning_uri(secret, current_user.email)
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
async def sso_callback(provider: str, request: Request, session: AsyncSession = Depends(get_session)):
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

    access_token, refresh_token = await issue_session(session, user)
    return SessionResponse(access_token=access_token, refresh_token=refresh_token)
