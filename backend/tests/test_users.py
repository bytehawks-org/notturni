from collections.abc import Callable

from httpx import AsyncClient

from tests.conftest import AuthedUser, FakeS3Client


async def test_public_profile(client: AsyncClient, make_user: Callable) -> None:
    await make_user("profilo1")
    res = await client.get("/api/v1/users/profilo1")
    assert res.status_code == 200
    body = res.json()
    assert body == {
        "username": "profilo1",
        "bio": None,
        "avatar_url": None,
        "social_links": [],
        "created_at": body["created_at"],
    }

    missing_res = await client.get("/api/v1/users/non-esiste")
    assert missing_res.status_code == 404


async def test_update_bio(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()
    res = await client.patch("/api/v1/users/me", json={"bio": "Ciao, sono io."}, headers=user.headers)
    assert res.status_code == 200
    assert res.json()["bio"] == "Ciao, sono io."


async def test_social_links_add_limit_delete(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user()

    for i in range(5):
        res = await client.post(
            "/api/v1/users/me/social-links",
            json={"label": f"Link{i}", "url": f"https://example.com/{i}"},
            headers=user.headers,
        )
        assert res.status_code == 201, res.text

    over_limit_res = await client.post(
        "/api/v1/users/me/social-links",
        json={"label": "Extra", "url": "https://example.com/extra"},
        headers=user.headers,
    )
    assert over_limit_res.status_code == 400

    profile_res = await client.get(f"/api/v1/users/{user.username}")
    links = profile_res.json()["social_links"]
    assert len(links) == 5

    delete_res = await client.delete(f"/api/v1/users/me/social-links/{links[0]['id']}", headers=user.headers)
    assert delete_res.status_code == 204

    profile_res2 = await client.get(f"/api/v1/users/{user.username}")
    assert len(profile_res2.json()["social_links"]) == 4


async def test_delete_social_link_of_others_forbidden(client: AsyncClient, make_user: Callable) -> None:
    owner: AuthedUser = await make_user("owner-link")
    stranger: AuthedUser = await make_user("stranger-link")
    create_res = await client.post(
        "/api/v1/users/me/social-links",
        json={"label": "Mio", "url": "https://example.com"},
        headers=owner.headers,
    )
    link_id = create_res.json()["id"]

    res = await client.delete(f"/api/v1/users/me/social-links/{link_id}", headers=stranger.headers)
    assert res.status_code == 404


async def test_avatar_upload_replace_delete(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    user: AuthedUser = await make_user()
    png_bytes = b"\x89PNG\r\n\x1a\n" + b"0" * 16

    upload_res = await client.post(
        "/api/v1/users/me/avatar",
        files={"file": ("avatar.png", png_bytes, "image/png")},
        headers=user.headers,
    )
    assert upload_res.status_code == 200
    first_url = upload_res.json()["avatar_url"]
    assert first_url is not None
    assert len(fake_s3.objects) == 1

    profile_res = await client.get(f"/api/v1/users/{user.username}")
    assert profile_res.json()["avatar_url"] == first_url

    # un secondo upload sostituisce e cancella il precedente
    second_res = await client.post(
        "/api/v1/users/me/avatar",
        files={"file": ("avatar2.png", png_bytes, "image/png")},
        headers=user.headers,
    )
    assert second_res.status_code == 200
    assert second_res.json()["avatar_url"] != first_url
    assert len(fake_s3.objects) == 1  # il vecchio è stato eliminato

    delete_res = await client.delete("/api/v1/users/me/avatar", headers=user.headers)
    assert delete_res.status_code == 204
    assert len(fake_s3.objects) == 0

    profile_res2 = await client.get(f"/api/v1/users/{user.username}")
    assert profile_res2.json()["avatar_url"] is None


async def test_avatar_rejects_invalid_content_type(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    user: AuthedUser = await make_user()
    res = await client.post(
        "/api/v1/users/me/avatar",
        files={"file": ("notes.txt", b"non e' un'immagine", "text/plain")},
        headers=user.headers,
    )
    assert res.status_code == 400
    assert len(fake_s3.objects) == 0


async def test_avatar_rejects_oversized_file(
    client: AsyncClient, make_user: Callable, fake_s3: FakeS3Client
) -> None:
    user: AuthedUser = await make_user()
    too_big = b"0" * (2 * 1024 * 1024 + 1)
    res = await client.post(
        "/api/v1/users/me/avatar",
        files={"file": ("avatar.png", too_big, "image/png")},
        headers=user.headers,
    )
    assert res.status_code == 400


async def test_user_follow_unfollow(client: AsyncClient, make_user: Callable) -> None:
    a: AuthedUser = await make_user("segue-a")
    b: AuthedUser = await make_user("segue-b")

    self_follow_res = await client.post(f"/api/v1/users/{a.username}/follow", headers=a.headers)
    assert self_follow_res.status_code == 400

    follow_res = await client.post(f"/api/v1/users/{b.username}/follow", headers=a.headers)
    assert follow_res.status_code == 204
    again_res = await client.post(f"/api/v1/users/{b.username}/follow", headers=a.headers)
    assert again_res.status_code == 204

    followers_res = await client.get(f"/api/v1/users/{b.username}/followers")
    assert [f["username"] for f in followers_res.json()] == [a.username]

    following_res = await client.get(f"/api/v1/users/{a.username}/following")
    assert [f["username"] for f in following_res.json()] == [b.username]

    unfollow_res = await client.delete(f"/api/v1/users/{b.username}/follow", headers=a.headers)
    assert unfollow_res.status_code == 204
    followers_res2 = await client.get(f"/api/v1/users/{b.username}/followers")
    assert followers_res2.json() == []
