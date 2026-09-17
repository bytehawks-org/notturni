"""Protezione SSRF dell'anteprima link (app/domain/link_preview.py) — schema
ammesso, IP privati/riservati bloccati, pinning IP per chiudere la finestra
di DNS rebinding fra la validazione e la richiesta effettiva."""

import socket

import pytest

from app.domain.link_preview import _is_blocked_ip, _resolve_pinned_ip, validate_previewable_url


@pytest.mark.parametrize(
    "ip,blocked",
    [
        ("8.8.8.8", False),
        ("93.184.216.34", False),
        ("127.0.0.1", True),
        ("10.0.0.5", True),
        ("192.168.1.1", True),
        ("169.254.1.1", True),
        ("::1", True),
        ("224.0.0.1", True),
    ],
)
def test_is_blocked_ip(ip: str, blocked: bool) -> None:
    assert _is_blocked_ip(ip) is blocked


def _fake_getaddrinfo(addresses: list[str]):
    def _fake(hostname, *_args, **_kwargs):
        return [(socket.AF_INET, socket.SOCK_STREAM, 0, "", (ip, 0)) for ip in addresses]

    return _fake


def test_resolve_pinned_ip_returns_safe_address(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(socket, "getaddrinfo", _fake_getaddrinfo(["93.184.216.34"]))
    assert _resolve_pinned_ip("example.com") == "93.184.216.34"


def test_resolve_pinned_ip_rejects_when_only_private_addresses(monkeypatch: pytest.MonkeyPatch) -> None:
    """Il caso di DNS rebinding: se anche solo l'unico indirizzo risolto è
    privato, nessun pinning avviene — fetch_link_preview rinuncia invece di
    connettersi lì."""
    monkeypatch.setattr(socket, "getaddrinfo", _fake_getaddrinfo(["127.0.0.1"]))
    assert _resolve_pinned_ip("evil.example.com") is None


def test_resolve_pinned_ip_none_on_dns_failure(monkeypatch: pytest.MonkeyPatch) -> None:
    def _raise(*_args, **_kwargs):
        raise OSError("resolution failed")

    monkeypatch.setattr(socket, "getaddrinfo", _raise)
    assert _resolve_pinned_ip("does-not-resolve.invalid") is None


def test_validate_previewable_url_rejects_non_http_scheme() -> None:
    with pytest.raises(ValueError):
        validate_previewable_url("ftp://example.com/file")


def test_validate_previewable_url_rejects_missing_host() -> None:
    with pytest.raises(ValueError):
        validate_previewable_url("https:///no-host")


def test_validate_previewable_url_rejects_private_ip(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(socket, "getaddrinfo", _fake_getaddrinfo(["127.0.0.1"]))
    with pytest.raises(ValueError):
        validate_previewable_url("http://internal.example.com/")


def test_validate_previewable_url_accepts_public_host(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(socket, "getaddrinfo", _fake_getaddrinfo(["93.184.216.34"]))
    assert validate_previewable_url("https://example.com/article") == "https://example.com/article"
