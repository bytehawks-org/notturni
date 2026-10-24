use crate::{
    BLOG_DELETE_GRACE_DAYS, BlogConfig, BlogCreate, BlogDelete, BlogOut, BlogTransfer, BlogUpdate, INVITABLE_ROLES,
    InvitationCreate, InvitationOut, MemberOut, MemberRoleUpdate, MembershipBlogOut, MyMembershipUpdate, rules, view,
};
use chrono::{Duration, Utc};
use noct_audit::{Entry, user_actor};
use noct_core::{
    Actor, ApiError, AppState, BlogRef, Entity, ExtCtx, Op, RouteCtx, body_map, can_view_blog, load_blog, map_db_error,
    parse_uuid, revalidate,
};
use serde_json::{Value, json};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use uuid::Uuid;

fn actor(ctx: &RouteCtx) -> Result<Actor, ApiError> {
    ctx.actor.clone().ok_or_else(|| ApiError::unauthorized("Not authenticated"))
}

fn audit_err(e: noct_audit::AuditError) -> ApiError {
    ApiError::internal(e)
}

/// Riga di audit con l'utente della richiesta come attore, nella transazione dell'azione.
async fn audit(
    conn: &mut PgConnection,
    ctx: &RouteCtx,
    user: Uuid,
    action: &str,
    target: (&str, Uuid),
    blog: Uuid,
    payload: Value,
) -> Result<(), ApiError> {
    let actor = user_actor(&mut *conn, user).await.map_err(audit_err)?;
    noct_audit::record(
        conn,
        Entry { action, actor, target: Some(target), blog_id: Some(blog), meta: &ctx.meta, payload },
    )
    .await
    .map_err(audit_err)
}

fn slug_param(ctx: &RouteCtx) -> Result<&str, ApiError> {
    ctx.path.get("slug").map(String::as_str).ok_or_else(|| ApiError::internal("parametro `slug` assente"))
}

async fn blog_or_404(st: &AppState, slug: &str) -> Result<BlogRef, ApiError> {
    load_blog(st, slug).await?.ok_or_else(|| ApiError::not_found("Blog non trovato."))
}

/// Il blog, solo se `user` ne è il proprietario (404 se manca, 403 altrimenti).
async fn owned_blog(st: &AppState, slug: &str, user: Uuid, denied: &str) -> Result<BlogRef, ApiError> {
    let blog = blog_or_404(st, slug).await?;
    if blog.owner_id != user {
        return Err(ApiError::forbidden(denied));
    }
    Ok(blog)
}

fn revalidate_blog(st: &AppState, slug: &str) {
    st.services.revalidate.spawn(vec![revalidate::blog(slug), revalidate::feed()]);
}

async fn count_owned(conn: &mut PgConnection, owner: Uuid) -> Result<i64, ApiError> {
    Ok(sqlx::query_scalar("SELECT count(*) FROM blogs WHERE owner_id = $1").bind(owner).fetch_one(conn).await?)
}

fn limit_message(max: i32) -> String {
    format!("Limite massimo di {max} blog per utente raggiunto.")
}

// ---- creazione ed elenchi ---------------------------------------------------------------------

pub async fn create_blog(ctx: RouteCtx, body: BlogCreate) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let platform = noct_platform::get(&st.db).await?;
    rules::validate_slug(&body.slug, &platform.reserved_blog_names).map_err(ApiError::bad_request)?;
    let locale = body.default_locale.clone().unwrap_or_else(|| "it".into());
    rules::validate_locale(&locale).map_err(ApiError::bad_request)?;
    let subtitle = body.subtitle.clone().filter(|s| !s.is_empty());
    let description = body.description.clone().filter(|s| !s.is_empty());
    if let Some(s) = &subtitle {
        rules::validate_subtitle(s).map_err(ApiError::bad_request)?;
    }
    if let Some(d) = &description {
        rules::validate_description(d).map_err(ApiError::bad_request)?;
    }

    let mut tx = st.db.begin().await?;
    // serializza le creazioni dello stesso utente: il conteggio e l'inserimento non devono avere corse
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE").bind(actor.user_id).execute(&mut *tx).await?;
    if count_owned(&mut tx, actor.user_id).await? >= i64::from(platform.max_blogs_per_user) {
        return Err(ApiError::bad_request(limit_message(platform.max_blogs_per_user)));
    }
    let visibility = body.visibility.clone().unwrap_or_else(|| "public".into());
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO blogs (slug, title, subtitle, description, owner_id, visibility, default_locale, default_author_display_name)
         VALUES ($1, $2, $3, $4, $5, $6::blog_visibility, $7, $8) RETURNING id",
    )
    .bind(&body.slug)
    .bind(&body.title)
    .bind(&subtitle)
    .bind(&description)
    .bind(actor.user_id)
    .bind(&visibility)
    .bind(&locale)
    .bind(body.default_author_display_name.clone().filter(|s| !s.is_empty()))
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| map_db_error(e, &[("uq_blogs_slug", 409, "Slug già in uso.")]))?;

    let blog_ref = BlogRef {
        id,
        slug: body.slug.clone(),
        owner_id: actor.user_id,
        visibility,
        is_suspended: false,
        is_paused: false,
        deleted_at: None,
    };
    st.extensions
        .apply(
            &mut tx,
            Entity::Blog,
            &ExtCtx { op: Op::Create, entity_id: id, source_id: None, blog: &blog_ref, actor: &actor },
            &body_map(&ctx.body),
        )
        .await?;
    audit(&mut tx, &ctx, actor.user_id, "blog.created", ("blog", id), id, json!({ "slug": body.slug })).await?;
    tx.commit().await?;
    view::render_one(st, id, Some(actor.user_id)).await
}

pub async fn list_my_blogs(ctx: RouteCtx) -> Result<Vec<Value>, ApiError> {
    let actor = actor(&ctx)?;
    let ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM blogs WHERE owner_id = $1 ORDER BY created_at, id")
        .bind(actor.user_id)
        .fetch_all(&ctx.state.db)
        .await?;
    let rows = view::fetch(&ctx.state, &ids).await?;
    view::render(&ctx.state, rows, Some(actor.user_id)).await
}

pub async fn list_blogs_member_of(ctx: RouteCtx) -> Result<Vec<MembershipBlogOut>, ApiError> {
    let actor = actor(&ctx)?;
    let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT bm.blog_id, bm.role::text, bm.author_display_name
         FROM blog_memberships bm JOIN blogs b ON b.id = bm.blog_id
         WHERE bm.user_id = $1 ORDER BY b.title, b.id",
    )
    .bind(actor.user_id)
    .fetch_all(&ctx.state.db)
    .await?;
    let ids: Vec<Uuid> = rows.iter().map(|r| r.0).collect();
    let blogs = view::fetch(&ctx.state, &ids).await?;
    let rendered = view::render(&ctx.state, blogs, Some(actor.user_id)).await?;
    Ok(rows
        .into_iter()
        .zip(rendered)
        .map(|((_, role, author_display_name), blog)| MembershipBlogOut { blog, role, author_display_name })
        .collect())
}

// ---- dettaglio e modifica --------------------------------------------------------------------

pub async fn get_blog(ctx: RouteCtx) -> Result<Value, ApiError> {
    let st = &ctx.state;
    let blog = blog_or_404(st, slug_param(&ctx)?).await?;
    let viewer = ctx.actor.as_ref().map(|a| a.user_id);
    // Un blog pubblico sospeso o in pausa resta leggibile nel dettaglio, così la pagina può
    // spiegare lo stato; post e contenuti restano 404.
    let readable_state_page = blog.deleted_at.is_none() && blog.visibility == "public";
    if !readable_state_page && !can_view_blog(st, viewer, &blog).await? {
        return Err(ApiError::not_found("Blog non trovato."));
    }
    view::render_one(st, blog.id, viewer).await
}

pub async fn update_blog(ctx: RouteCtx, body: BlogUpdate) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può modificare il blog.").await?;
    if let Some(s) = body.subtitle.as_deref().filter(|s| !s.is_empty()) {
        rules::validate_subtitle(s).map_err(ApiError::bad_request)?;
    }
    if let Some(d) = body.description.as_deref().filter(|s| !s.is_empty()) {
        rules::validate_description(d).map_err(ApiError::bad_request)?;
    }

    let mut tx = st.db.begin().await?;
    let default_locale: String = sqlx::query_scalar("SELECT default_locale FROM blogs WHERE id = $1 FOR UPDATE")
        .bind(blog.id)
        .fetch_one(&mut *tx)
        .await?;
    let extra_locales = match &body.extra_locales {
        Some(list) => Some(rules::normalize_extra_locales(list, &default_locale).map_err(ApiError::bad_request)?),
        None => None,
    };

    let mut qb = QueryBuilder::<Postgres>::new("UPDATE blogs SET updated_at = now()");
    if let Some(v) = &body.title {
        qb.push(", title = ").push_bind(v);
    }
    // "" azzera, assente lascia invariato
    if let Some(v) = &body.subtitle {
        qb.push(", subtitle = ").push_bind((!v.is_empty()).then(|| v.clone()));
    }
    if let Some(v) = &body.description {
        qb.push(", description = ").push_bind((!v.is_empty()).then(|| v.clone()));
    }
    if let Some(v) = &body.visibility {
        qb.push(", visibility = ").push_bind(v).push("::blog_visibility");
    }
    if let Some(v) = body.mentions_enabled {
        qb.push(", mentions_enabled = ").push_bind(v);
    }
    if let Some(v) = body.search_indexing_enabled {
        qb.push(", search_indexing_enabled = ").push_bind(v);
    }
    if let Some(v) = body.ai_crawling_enabled {
        qb.push(", ai_crawling_enabled = ").push_bind(v);
    }
    if let Some(v) = &body.default_author_display_name {
        qb.push(", default_author_display_name = ").push_bind((!v.is_empty()).then(|| v.clone()));
    }
    if let Some(v) = body.is_paused {
        qb.push(", is_paused = ").push_bind(v);
    }
    if let Some(v) = extra_locales {
        qb.push(", extra_locales = ").push_bind(v);
    }
    qb.push(" WHERE id = ").push_bind(blog.id);
    qb.build().execute(&mut *tx).await?;

    st.extensions
        .apply(
            &mut tx,
            Entity::Blog,
            &ExtCtx { op: Op::Update, entity_id: blog.id, source_id: None, blog: &blog, actor: &actor },
            &body_map(&ctx.body),
        )
        .await?;
    tx.commit().await?;
    revalidate_blog(st, &blog.slug);
    view::render_one(st, blog.id, Some(actor.user_id)).await
}

// ---- ciclo di vita --------------------------------------------------------------------------------

pub async fn delete_blog(ctx: RouteCtx, body: BlogDelete) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può farlo.").await?;
    if body.confirm_slug.trim() != blog.slug {
        return Err(ApiError::bad_request("Lo slug digitato non corrisponde."));
    }
    if blog.deleted_at.is_none() {
        let mut tx = st.db.begin().await?;
        let deleted_at = Utc::now();
        // idempotente anche con due richieste concorrenti: solo la prima scrive e controlla
        let done =
            sqlx::query("UPDATE blogs SET deleted_at = $2, updated_at = now() WHERE id = $1 AND deleted_at IS NULL")
                .bind(blog.id)
                .bind(deleted_at)
                .execute(&mut *tx)
                .await?;
        if done.rows_affected() == 1 {
            audit(&mut tx, &ctx, actor.user_id, "blog.deleted", ("blog", blog.id), blog.id, json!({ "slug": blog.slug, "purge_after": (deleted_at + Duration::days(BLOG_DELETE_GRACE_DAYS)).to_rfc3339(), })).await?;
        }
        tx.commit().await?;
        revalidate_blog(st, &blog.slug);
    }
    view::render_one(st, blog.id, Some(actor.user_id)).await
}

pub async fn restore_blog(ctx: RouteCtx) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può farlo.").await?;
    if blog.deleted_at.is_some() {
        let mut tx = st.db.begin().await?;
        let done = sqlx::query(
            "UPDATE blogs SET deleted_at = NULL, updated_at = now() WHERE id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(blog.id)
        .execute(&mut *tx)
        .await?;
        if done.rows_affected() == 1 {
            audit(
                &mut tx,
                &ctx,
                actor.user_id,
                "blog.restored",
                ("blog", blog.id),
                blog.id,
                json!({ "slug": blog.slug }),
            )
            .await?;
        }
        tx.commit().await?;
        revalidate_blog(st, &blog.slug);
    }
    view::render_one(st, blog.id, Some(actor.user_id)).await
}

pub async fn transfer_blog(ctx: RouteCtx, body: BlogTransfer) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può farlo.").await?;
    let target: Option<(Uuid, bool)> = sqlx::query_as("SELECT id, is_active FROM users WHERE username = $1")
        .bind(&body.username)
        .fetch_optional(&st.db)
        .await?;
    let Some((target_id, true)) = target else {
        return Err(ApiError::not_found("Utente non trovato."));
    };
    if target_id == actor.user_id {
        return Err(ApiError::bad_request("Sei già il proprietario."));
    }
    let platform = noct_platform::get(&st.db).await?;

    let mut tx = st.db.begin().await?;
    // blocca il blog e il destinatario: proprietà e conteggio non devono cambiare sotto i piedi
    let owner: Uuid = sqlx::query_scalar("SELECT owner_id FROM blogs WHERE id = $1 FOR UPDATE")
        .bind(blog.id)
        .fetch_one(&mut *tx)
        .await?;
    if owner != actor.user_id {
        return Err(ApiError::forbidden("Solo il proprietario può farlo."));
    }
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE").bind(target_id).execute(&mut *tx).await?;
    let role: Option<String> =
        sqlx::query_scalar("SELECT role::text FROM blog_memberships WHERE blog_id = $1 AND user_id = $2")
            .bind(blog.id)
            .bind(target_id)
            .fetch_optional(&mut *tx)
            .await?;
    if role.as_deref() != Some("co_autore") {
        return Err(ApiError::bad_request("Il nuovo proprietario deve essere un coautore del blog."));
    }
    if count_owned(&mut tx, target_id).await? >= i64::from(platform.max_blogs_per_user) {
        return Err(ApiError::bad_request(format!(
            "Il nuovo proprietario: {}",
            limit_message(platform.max_blogs_per_user)
        )));
    }
    sqlx::query("DELETE FROM blog_memberships WHERE blog_id = $1 AND user_id = $2")
        .bind(blog.id)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE blogs SET owner_id = $2, updated_at = now() WHERE id = $1")
        .bind(blog.id)
        .bind(target_id)
        .execute(&mut *tx)
        .await?;
    // chi cede resta come coautore
    sqlx::query("INSERT INTO blog_memberships (blog_id, user_id, role) VALUES ($1, $2, 'co_autore')")
        .bind(blog.id)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
    audit(
        &mut tx,
        &ctx,
        actor.user_id,
        "blog.ownership_transferred",
        ("blog", blog.id),
        blog.id,
        json!({ "slug": blog.slug, "to": body.username }),
    )
    .await?;
    tx.commit().await?;
    view::render_one(st, blog.id, Some(actor.user_id)).await
}

// ---- collaboratori -----------------------------------------------------------------------------------

const MEMBER_SELECT: &str = "SELECT bm.user_id, u.username, u.avatar_object_key AS avatar_url, bm.role::text AS role, \
    bm.author_display_name, bm.created_at \
    FROM blog_memberships bm JOIN users u ON u.id = bm.user_id";

fn resolve_avatar(st: &AppState, mut m: MemberOut) -> MemberOut {
    m.avatar_url = m.avatar_url.take().map(|k| st.services.storage.public_url(&st.config.storage.bucket_avatars, &k));
    m
}

pub async fn list_members(ctx: RouteCtx) -> Result<Vec<MemberOut>, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può gestire i collaboratori.").await?;
    let rows: Vec<MemberOut> = sqlx::query_as(&format!("{MEMBER_SELECT} WHERE bm.blog_id = $1 ORDER BY u.username"))
        .bind(blog.id)
        .fetch_all(&st.db)
        .await?;
    Ok(rows.into_iter().map(|m| resolve_avatar(st, m)).collect())
}

pub async fn update_member(ctx: RouteCtx, body: MemberRoleUpdate) -> Result<MemberOut, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può gestire i collaboratori.").await?;
    let user_id = parse_uuid(ctx.path.get("user_id").map(String::as_str).unwrap_or_default())?;
    if !INVITABLE_ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::bad_request("Ruolo assegnabile solo co_autore o mediatore da questa interfaccia."));
    }
    let mut tx = st.db.begin().await?;
    let old: Option<String> =
        sqlx::query_scalar("SELECT role::text FROM blog_memberships WHERE blog_id = $1 AND user_id = $2 FOR UPDATE")
            .bind(blog.id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(old) = old else {
        return Err(ApiError::not_found("Collaboratore non trovato."));
    };
    sqlx::query(
        "UPDATE blog_memberships SET role = $3::blog_role, updated_at = now() WHERE blog_id = $1 AND user_id = $2",
    )
    .bind(blog.id)
    .bind(user_id)
    .bind(&body.role)
    .execute(&mut *tx)
    .await?;
    audit(
        &mut tx,
        &ctx,
        actor.user_id,
        "blog.member_role_changed",
        ("user", user_id),
        blog.id,
        json!({ "from": old, "to": body.role, "blog_slug": blog.slug }),
    )
    .await?;
    tx.commit().await?;
    let row: MemberOut = sqlx::query_as(&format!("{MEMBER_SELECT} WHERE bm.blog_id = $1 AND bm.user_id = $2"))
        .bind(blog.id)
        .bind(user_id)
        .fetch_one(&st.db)
        .await?;
    Ok(resolve_avatar(st, row))
}

pub async fn remove_member(ctx: RouteCtx) -> Result<(), ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può gestire i collaboratori.").await?;
    let user_id = parse_uuid(ctx.path.get("user_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = st.db.begin().await?;
    let removed: Option<String> =
        sqlx::query_scalar("DELETE FROM blog_memberships WHERE blog_id = $1 AND user_id = $2 RETURNING role::text")
            .bind(blog.id)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Some(role) = removed {
        audit(
            &mut tx,
            &ctx,
            actor.user_id,
            "blog.member_removed",
            ("user", user_id),
            blog.id,
            json!({ "role": role, "blog_slug": blog.slug }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn update_my_membership(ctx: RouteCtx, body: MyMembershipUpdate) -> Result<MembershipBlogOut, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog = blog_or_404(st, slug_param(&ctx)?).await?;
    let mut qb = QueryBuilder::<Postgres>::new("UPDATE blog_memberships SET updated_at = now()");
    if let Some(v) = &body.author_display_name {
        qb.push(", author_display_name = ").push_bind((!v.is_empty()).then(|| v.clone()));
    }
    qb.push(" WHERE blog_id = ").push_bind(blog.id).push(" AND user_id = ").push_bind(actor.user_id);
    qb.push(" RETURNING role::text, author_display_name");
    let row: Option<(String, Option<String>)> = qb.build_query_as().fetch_optional(&st.db).await?;
    let Some((role, author_display_name)) = row else {
        return Err(ApiError::not_found("Non sei un collaboratore di questo blog."));
    };
    Ok(MembershipBlogOut { blog: view::render_one(st, blog.id, Some(actor.user_id)).await?, role, author_display_name })
}

// ---- inviti ----------------------------------------------------------------------------------------------

const INVITATION_SELECT: &str = "SELECT i.id, b.slug AS blog_slug, b.title AS blog_title, i.role::text AS role, \
    i.status::text AS status, iu.username AS invited_username, bu.username AS invited_by_username, \
    i.created_at, i.responded_at \
    FROM blog_invitations i \
    JOIN blogs b ON b.id = i.blog_id \
    JOIN users iu ON iu.id = i.invited_user_id \
    JOIN users bu ON bu.id = i.invited_by_id";

async fn invitation_out(conn: &mut PgConnection, id: Uuid) -> Result<InvitationOut, ApiError> {
    Ok(sqlx::query_as(&format!("{INVITATION_SELECT} WHERE i.id = $1")).bind(id).fetch_one(conn).await?)
}

pub async fn list_received_invitations(ctx: RouteCtx) -> Result<Vec<InvitationOut>, ApiError> {
    let actor = actor(&ctx)?;
    Ok(sqlx::query_as(&format!(
        "{INVITATION_SELECT} WHERE i.invited_user_id = $1 AND i.status = 'pending' ORDER BY i.created_at DESC"
    ))
    .bind(actor.user_id)
    .fetch_all(&ctx.state.db)
    .await?)
}

/// Carica e blocca un invito ricevuto ancora in attesa.
async fn pending_received(conn: &mut PgConnection, id: Uuid, user: Uuid) -> Result<(Uuid, String), ApiError> {
    let row: Option<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT blog_id, invited_user_id, role::text, status::text FROM blog_invitations WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    match row {
        Some((blog_id, invited, role, status)) if invited == user => {
            if status != "pending" {
                return Err(ApiError::conflict("Invito già gestito."));
            }
            Ok((blog_id, role))
        }
        _ => Err(ApiError::not_found("Invito non trovato.")),
    }
}

pub async fn accept_invitation(ctx: RouteCtx) -> Result<InvitationOut, ApiError> {
    let actor = actor(&ctx)?;
    let id = parse_uuid(ctx.path.get("invitation_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = ctx.state.db.begin().await?;
    let (blog_id, role) = pending_received(&mut tx, id, actor.user_id).await?;
    // già membro per altra via: si allinea il ruolo invece di fallire
    sqlx::query(
        "INSERT INTO blog_memberships (blog_id, user_id, role) VALUES ($1, $2, $3::blog_role)
         ON CONFLICT (blog_id, user_id) DO UPDATE SET role = EXCLUDED.role, updated_at = now()",
    )
    .bind(blog_id)
    .bind(actor.user_id)
    .bind(&role)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE blog_invitations SET status = 'accepted', responded_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    audit(
        &mut tx,
        &ctx,
        actor.user_id,
        "blog.invitation_accepted",
        ("invitation", id),
        blog_id,
        json!({ "role": role }),
    )
    .await?;
    let out = invitation_out(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn decline_invitation(ctx: RouteCtx) -> Result<InvitationOut, ApiError> {
    let actor = actor(&ctx)?;
    let id = parse_uuid(ctx.path.get("invitation_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = ctx.state.db.begin().await?;
    let (blog_id, role) = pending_received(&mut tx, id, actor.user_id).await?;
    sqlx::query(
        "UPDATE blog_invitations SET status = 'declined', responded_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;
    audit(
        &mut tx,
        &ctx,
        actor.user_id,
        "blog.invitation_declined",
        ("invitation", id),
        blog_id,
        json!({ "role": role }),
    )
    .await?;
    let out = invitation_out(&mut tx, id).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn list_invitations(ctx: RouteCtx) -> Result<Vec<InvitationOut>, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può gestire i collaboratori.").await?;
    Ok(sqlx::query_as(&format!("{INVITATION_SELECT} WHERE i.blog_id = $1 ORDER BY i.created_at DESC"))
        .bind(blog.id)
        .fetch_all(&st.db)
        .await?)
}

pub async fn create_invitation(ctx: RouteCtx, body: InvitationCreate) -> Result<InvitationOut, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può gestire i collaboratori.").await?;
    if !INVITABLE_ROLES.contains(&body.role.as_str()) {
        return Err(ApiError::bad_request("Si può invitare solo come co_autore o mediatore."));
    }
    let invited: Option<(Uuid, String, bool)> =
        sqlx::query_as("SELECT id, username, is_active FROM users WHERE username = $1")
            .bind(&body.username)
            .fetch_optional(&st.db)
            .await?;
    let Some((invited_id, invited_name, true)) = invited else {
        return Err(ApiError::not_found("Utente non trovato."));
    };
    if invited_id == blog.owner_id {
        return Err(ApiError::bad_request("Sei già il proprietario del blog."));
    }

    let mut tx = st.db.begin().await?;
    // serializza gli inviti allo stesso utente sullo stesso blog
    sqlx::query("SELECT id FROM blogs WHERE id = $1 FOR UPDATE").bind(blog.id).execute(&mut *tx).await?;
    let member: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM blog_memberships WHERE blog_id = $1 AND user_id = $2)")
            .bind(blog.id)
            .bind(invited_id)
            .fetch_one(&mut *tx)
            .await?;
    if member {
        return Err(ApiError::conflict("L'utente è già un collaboratore."));
    }
    let existing: Option<(Uuid, String)> =
        sqlx::query_as("SELECT id, status::text FROM blog_invitations WHERE blog_id = $1 AND invited_user_id = $2")
            .bind(blog.id)
            .bind(invited_id)
            .fetch_optional(&mut *tx)
            .await?;
    let invitation_id = match existing {
        Some((_, status)) if status == "pending" => {
            return Err(ApiError::conflict("C'è già un invito in attesa per questo utente."));
        }
        // riusa la riga di un invito rifiutato o revocato in precedenza
        Some((id, _)) => {
            sqlx::query(
                "UPDATE blog_invitations SET role = $2::blog_role, invited_by_id = $3, status = 'pending',
                 responded_at = NULL, updated_at = now() WHERE id = $1",
            )
            .bind(id)
            .bind(&body.role)
            .bind(actor.user_id)
            .execute(&mut *tx)
            .await?;
            id
        }
        None => {
            sqlx::query_scalar(
                "INSERT INTO blog_invitations (blog_id, invited_user_id, invited_by_id, role)
                 VALUES ($1, $2, $3, $4::blog_role) RETURNING id",
            )
            .bind(blog.id)
            .bind(invited_id)
            .bind(actor.user_id)
            .bind(&body.role)
            .fetch_one(&mut *tx)
            .await?
        }
    };
    audit(
        &mut tx,
        &ctx,
        actor.user_id,
        "blog.invitation_created",
        ("invitation", invitation_id),
        blog.id,
        json!({ "invited_username": invited_name, "role": body.role, "blog_slug": blog.slug }),
    )
    .await?;
    let out = invitation_out(&mut tx, invitation_id).await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn revoke_invitation(ctx: RouteCtx) -> Result<(), ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può gestire i collaboratori.").await?;
    let id = parse_uuid(ctx.path.get("invitation_id").map(String::as_str).unwrap_or_default())?;
    let mut tx = st.db.begin().await?;
    let row: Option<(Uuid, String, Uuid, String)> = sqlx::query_as(
        "SELECT blog_id, status::text, invited_user_id, role::text FROM blog_invitations WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((blog_id, status, invited_user, role)) = row.filter(|r| r.0 == blog.id) else {
        return Err(ApiError::not_found("Invito non trovato."));
    };
    let _ = blog_id;
    if status == "pending" {
        sqlx::query(
            "UPDATE blog_invitations SET status = 'revoked', responded_at = now(), updated_at = now() WHERE id = $1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        audit(
            &mut tx,
            &ctx,
            actor.user_id,
            "blog.invitation_revoked",
            ("invitation", id),
            blog.id,
            json!({ "invited_user_id": invited_user.to_string(), "role": role, "blog_slug": blog.slug }),
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

// ---- configurazione di presentazione ---------------------------------------------------------------------

pub async fn get_config(ctx: RouteCtx) -> Result<Value, ApiError> {
    let blog = ctx.blog.as_ref().ok_or_else(|| ApiError::internal("scope del blog assente"))?;
    let stored: Option<Value> = sqlx::query_scalar("SELECT config FROM blog_configs WHERE blog_id = $1")
        .bind(blog.id)
        .fetch_optional(&ctx.state.db)
        .await?;
    Ok(stored.unwrap_or_else(rules::default_config))
}

pub async fn put_config(ctx: RouteCtx, _body: BlogConfig) -> Result<Value, ApiError> {
    let actor = actor(&ctx)?;
    let st = &ctx.state;
    let blog =
        owned_blog(st, slug_param(&ctx)?, actor.user_id, "Solo il proprietario può modificare la configurazione.")
            .await?;
    // si conserva il documento così com'è, comprese le chiavi che questo schema non conosce
    let config = body_map(&ctx.body);
    rules::validate_config(&config).map_err(ApiError::bad_request)?;
    let config = Value::Object(config);
    sqlx::query(
        "INSERT INTO blog_configs (blog_id, config) VALUES ($1, $2)
         ON CONFLICT (blog_id) DO UPDATE SET config = EXCLUDED.config, updated_at = now()",
    )
    .bind(blog.id)
    .bind(&config)
    .execute(&st.db)
    .await?;
    st.services.revalidate.spawn(vec![revalidate::blog(&blog.slug)]);
    Ok(config)
}

#[allow(dead_code)]
fn _types_are_used(_: &BlogOut) {}
