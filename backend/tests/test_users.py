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
        "first_name": None,
        "last_name": None,
        "display_name": None,
        "post_author_name_style": "username",
        "country": None,
        "native_language": None,
        "fallback_languages": [],
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


async def test_update_profile_bio_fields(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("profilo-bio")

    res = await client.patch(
        "/api/v1/users/me",
        json={
            "first_name": "Ada",
            "last_name": "Lovelace",
            "country": "gb",
            "native_language": "EN",
            "fallback_languages": ["It", "fr"],
        },
        headers=user.headers,
    )
    assert res.status_code == 200
    body = res.json()
    assert body["first_name"] == "Ada"
    assert body["last_name"] == "Lovelace"
    assert body["country"] == "GB"
    assert body["native_language"] == "en"
    assert body["fallback_languages"] == ["it", "fr"]

    # azzerare con stringa vuota
    clear_res = await client.patch(
        "/api/v1/users/me", json={"country": ""}, headers=user.headers
    )
    assert clear_res.json()["country"] is None
    # gli altri campi restano quelli già salvati
    assert clear_res.json()["first_name"] == "Ada"


async def test_update_profile_display_name(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("profilo-alias")

    set_res = await client.patch(
        "/api/v1/users/me", json={"display_name": "  Gatto Nero  "}, headers=user.headers
    )
    assert set_res.status_code == 200
    assert set_res.json()["display_name"] == "Gatto Nero"

    public = await client.get("/api/v1/users/profilo-alias")
    assert public.json()["display_name"] == "Gatto Nero"

    clear_res = await client.patch(
        "/api/v1/users/me", json={"display_name": ""}, headers=user.headers
    )
    assert clear_res.json()["display_name"] is None


async def test_post_author_name_style_choice(client: AsyncClient, make_user: Callable) -> None:
    user: AuthedUser = await make_user("stile-nome")
    await client.patch(
        "/api/v1/users/me",
        json={"first_name": "Ada", "last_name": "Byron", "display_name": "Contessa"},
        headers=user.headers,
    )
    await client.post("/api/v1/blogs", json={"slug": "blog-stile", "title": "x"}, headers=user.headers)

    async def author_of_new_post(slug: str) -> str:
        res = await client.post(
            "/api/v1/blogs/blog-stile/posts",
            json={"slug": slug, "title": "t", "content": "c"},
            headers=user.headers,
        )
        assert res.status_code == 201, res.text
        return res.json()["author_display_name"]

    # default: username
    assert await author_of_new_post("p-username") == "stile-nome"

    await client.patch(
        "/api/v1/users/me", json={"post_author_name_style": "full_name"}, headers=user.headers
    )
    assert (await client.get("/api/v1/users/stile-nome")).json()["post_author_name_style"] == "full_name"
    assert await author_of_new_post("p-fullname") == "Ada Byron"

    await client.patch(
        "/api/v1/users/me", json={"post_author_name_style": "display_name"}, headers=user.headers
    )
    assert await author_of_new_post("p-display") == "Contessa"

    # valore non valido → 422
    bad = await client.patch(
        "/api/v1/users/me", json={"post_author_name_style": "pseudonimo"}, headers=user.headers
    )
    assert bad.status_code == 422


async def test_post_author_name_style_ignored_when_blog_imposes_alias(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("stile-ignorato")
    await client.patch(
        "/api/v1/users/me",
        json={"first_name": "Ada", "post_author_name_style": "full_name"},
        headers=user.headers,
    )
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-impone", "title": "x"}, headers=user.headers
    )
    await client.patch(
        "/api/v1/blogs/blog-impone",
        json={"default_author_display_name": "La Voce"},
        headers=user.headers,
    )
    res = await client.post(
        "/api/v1/blogs/blog-impone/posts",
        json={"slug": "p", "title": "t", "content": "c"},
        headers=user.headers,
    )
    # l'alias del blog vince sulla preferenza di profilo, senza override
    assert res.json()["author_display_name"] == "La Voce"


async def test_resave_by_author_realigns_name_to_current_preference(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("risalva-nome")
    await client.post(
        "/api/v1/blogs", json={"slug": "blog-risalva", "title": "x"}, headers=user.headers
    )
    created = await client.post(
        "/api/v1/blogs/blog-risalva/posts",
        json={"slug": "p", "title": "t", "content": "c"},
        headers=user.headers,
    )
    post_id = created.json()["id"]
    assert created.json()["author_display_name"] == "risalva-nome"

    await client.patch(
        "/api/v1/users/me",
        json={"display_name": "Fenice", "post_author_name_style": "display_name"},
        headers=user.headers,
    )
    updated = await client.patch(
        f"/api/v1/posts/{post_id}", json={"content": "c2"}, headers=user.headers
    )
    assert updated.json()["author_display_name"] == "Fenice"


async def test_update_profile_rejects_invalid_country_and_language(
    client: AsyncClient, make_user: Callable
) -> None:
    user: AuthedUser = await make_user("profilo-bio-invalid")

    bad_country = await client.patch(
        "/api/v1/users/me", json={"country": "italy"}, headers=user.headers
    )
    assert bad_country.status_code == 400

    bad_language = await client.patch(
        "/api/v1/users/me", json={"native_language": "ita"}, headers=user.headers
    )
    assert bad_language.status_code == 400

    too_many = await client.patch(
        "/api/v1/users/me",
        json={"fallback_languages": ["it", "en", "fr", "de", "es", "pt"]},
        headers=user.headers,
    )
    assert too_many.status_code == 400
