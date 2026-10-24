from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser


async def _make_blog(client: AsyncClient, owner: AuthedUser, slug: str) -> None:
    res = await client.post(
        "/api/v1/blogs", json={"slug": slug, "title": "x"}, headers=owner.headers
    )
    assert res.status_code == 201, res.text


async def test_invitation_accept_creates_membership(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("inv-owner")
    guest: AuthedUser = await make_user("inv-guest")
    await _make_blog(client, owner, "blog-inviti")

    invite = await client.post(
        "/api/v1/blogs/blog-inviti/invitations",
        json={"username": "inv-guest", "role": "co_autore"},
        headers=owner.headers,
    )
    assert invite.status_code == 201, invite.text
    inv = invite.json()
    assert inv["status"] == "pending"
    assert inv["role"] == "co_autore"
    assert inv["invited_username"] == "inv-guest"
    assert inv["invited_by_username"] == "inv-owner"

    # l'invitato vede l'invito
    received = await client.get(
        "/api/v1/blogs/received-invitations", headers=guest.headers
    )
    assert received.status_code == 200
    assert [i["id"] for i in received.json()] == [inv["id"]]

    # ...e lo accetta
    accept = await client.post(
        f"/api/v1/blogs/received-invitations/{inv['id']}/accept", headers=guest.headers
    )
    assert accept.status_code == 200
    assert accept.json()["status"] == "accepted"

    # ora è membro: compare in member-of e può scrivere
    member_of = await client.get("/api/v1/blogs/member-of", headers=guest.headers)
    assert member_of.status_code == 200
    assert [m["blog"]["slug"] for m in member_of.json()] == ["blog-inviti"]
    assert member_of.json()[0]["role"] == "co_autore"

    post = await client.post(
        "/api/v1/blogs/blog-inviti/posts",
        json={"slug": "ciao", "title": "x", "content": "y"},
        headers=guest.headers,
    )
    assert post.status_code == 201, post.text

    # non resta più in attesa
    received2 = await client.get(
        "/api/v1/blogs/received-invitations", headers=guest.headers
    )
    assert received2.json() == []

    # il proprietario vede il collaboratore
    members = await client.get(
        "/api/v1/blogs/blog-inviti/members", headers=owner.headers
    )
    assert members.status_code == 200
    assert [m["username"] for m in members.json()] == ["inv-guest"]


async def test_invitation_decline_and_reinvite(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("inv2-owner")
    guest: AuthedUser = await make_user("inv2-guest")
    await _make_blog(client, owner, "blog-inviti-due")

    first = await client.post(
        "/api/v1/blogs/blog-inviti-due/invitations",
        json={"username": "inv2-guest", "role": "mediatore"},
        headers=owner.headers,
    )
    inv_id = first.json()["id"]

    # doppio invito in pending → 409
    dup = await client.post(
        "/api/v1/blogs/blog-inviti-due/invitations",
        json={"username": "inv2-guest", "role": "mediatore"},
        headers=owner.headers,
    )
    assert dup.status_code == 409

    decline = await client.post(
        f"/api/v1/blogs/received-invitations/{inv_id}/decline", headers=guest.headers
    )
    assert decline.status_code == 200
    assert decline.json()["status"] == "declined"

    # dopo il rifiuto si può reinvitare: riusa la stessa riga
    again = await client.post(
        "/api/v1/blogs/blog-inviti-due/invitations",
        json={"username": "inv2-guest", "role": "co_autore"},
        headers=owner.headers,
    )
    assert again.status_code == 201
    assert again.json()["id"] == inv_id
    assert again.json()["role"] == "co_autore"
    assert again.json()["status"] == "pending"


async def test_invitation_role_and_permission_rules(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("inv3-owner")
    stranger: AuthedUser = await make_user("inv3-stranger")
    await make_user("inv3-guest")
    await _make_blog(client, owner, "blog-inviti-tre")

    # ruolo non invitabile
    bad_role = await client.post(
        "/api/v1/blogs/blog-inviti-tre/invitations",
        json={"username": "inv3-guest", "role": "revisore"},
        headers=owner.headers,
    )
    assert bad_role.status_code == 400

    # utente inesistente
    missing = await client.post(
        "/api/v1/blogs/blog-inviti-tre/invitations",
        json={"username": "non-esiste", "role": "co_autore"},
        headers=owner.headers,
    )
    assert missing.status_code == 404

    # non proprietario non può invitare
    forbidden = await client.post(
        "/api/v1/blogs/blog-inviti-tre/invitations",
        json={"username": "inv3-guest", "role": "co_autore"},
        headers=stranger.headers,
    )
    assert forbidden.status_code == 403


async def test_revoke_invitation(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("inv4-owner")
    guest: AuthedUser = await make_user("inv4-guest")
    await _make_blog(client, owner, "blog-inviti-quattro")

    inv = await client.post(
        "/api/v1/blogs/blog-inviti-quattro/invitations",
        json={"username": "inv4-guest", "role": "co_autore"},
        headers=owner.headers,
    )
    inv_id = inv.json()["id"]

    revoke = await client.delete(
        f"/api/v1/blogs/blog-inviti-quattro/invitations/{inv_id}", headers=owner.headers
    )
    assert revoke.status_code == 204

    # non è più accettabile
    accept = await client.post(
        f"/api/v1/blogs/received-invitations/{inv_id}/accept", headers=guest.headers
    )
    assert accept.status_code == 409
    assert (
        await client.get("/api/v1/blogs/received-invitations", headers=guest.headers)
    ).json() == []


async def test_membership_alias_drives_post_author_name(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("alias-owner")
    guest: AuthedUser = await make_user("alias-guest")
    await _make_blog(client, owner, "blog-alias")
    await client.patch(
        "/api/v1/blogs/blog-alias",
        json={"default_author_display_name": "La Redazione"},
        headers=owner.headers,
    )
    # alias globale sul profilo dell'ospite
    await client.patch(
        "/api/v1/users/me", json={"display_name": "Alias Globale"}, headers=guest.headers
    )

    inv = await client.post(
        "/api/v1/blogs/blog-alias/invitations",
        json={"username": "alias-guest", "role": "co_autore"},
        headers=owner.headers,
    )
    await client.post(
        f"/api/v1/blogs/received-invitations/{inv.json()['id']}/accept",
        headers=guest.headers,
    )

    # senza alias per-blog: vince il default del blog
    p1 = await client.post(
        "/api/v1/blogs/blog-alias/posts",
        json={"slug": "uno", "title": "x", "content": "y"},
        headers=guest.headers,
    )
    assert p1.json()["author_display_name"] == "La Redazione"

    # il collaboratore imposta il proprio alias per questo blog
    my = await client.patch(
        "/api/v1/blogs/blog-alias/my-membership",
        json={"author_display_name": "Corvo"},
        headers=guest.headers,
    )
    assert my.status_code == 200
    assert my.json()["author_display_name"] == "Corvo"

    # ora l'alias per-blog ha la precedenza sul default del blog
    p2 = await client.post(
        "/api/v1/blogs/blog-alias/posts",
        json={"slug": "due", "title": "x", "content": "y"},
        headers=guest.headers,
    )
    assert p2.json()["author_display_name"] == "Corvo"

    # todo/USERS.md #2: con un alias imposto (qui quello di membership) non
    # c'è override per singolo post — il campo è stato rimosso dal payload e
    # viene comunque ignorato.
    p3 = await client.post(
        "/api/v1/blogs/blog-alias/posts",
        json={
            "slug": "tre",
            "title": "x",
            "content": "y",
            "author_display_name": "Nome Esplicito",
        },
        headers=guest.headers,
    )
    assert p3.json()["author_display_name"] == "Corvo"


async def test_owner_display_name_style_when_no_blog_alias(
    client: AsyncClient, make_user: Callable
) -> None:
    owner: AuthedUser = await make_user("solo-alias-owner")
    await client.patch(
        "/api/v1/users/me",
        json={"display_name": "Penna Solitaria", "post_author_name_style": "display_name"},
        headers=owner.headers,
    )
    await _make_blog(client, owner, "blog-solo-alias")

    post = await client.post(
        "/api/v1/blogs/blog-solo-alias/posts",
        json={"slug": "uno", "title": "x", "content": "y"},
        headers=owner.headers,
    )
    # nessun alias sul blog → si applica la preferenza di profilo (alias globale)
    assert post.json()["author_display_name"] == "Penna Solitaria"
