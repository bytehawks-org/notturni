"use client";

import Image from "next/image";
import { useRouter } from "next/navigation";
import { useEffect, useState, type FormEvent } from "react";

import { LanguagePicker } from "@/components/LanguagePicker";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { SOCIAL_PLATFORMS, getSocialPlatform } from "@/lib/social-platforms";
import {
  POST_AUTHOR_NAME_STYLE_LABELS,
  type FollowStats,
  type PostAuthorNameStyle,
  type Profile,
} from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function ProfilePage() {
  const router = useRouter();
  const { user, authFetch, refreshUser, logout } = useAuth();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [username, setUsername] = useState("");
  const [bio, setBio] = useState("");
  const [firstName, setFirstName] = useState("");
  const [lastName, setLastName] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [authorNameStyle, setAuthorNameStyle] = useState<PostAuthorNameStyle>("username");
  const [country, setCountry] = useState("");
  const [nativeLanguage, setNativeLanguage] = useState<string | null>(null);
  const [fallbackLanguages, setFallbackLanguages] = useState<string[]>([]);
  const [savingBio, setSavingBio] = useState(false);

  const [linkPlatform, setLinkPlatform] = useState(SOCIAL_PLATFORMS[0].key);
  const [linkUrl, setLinkUrl] = useState("");
  const [linkError, setLinkError] = useState<string | null>(null);

  const [avatarError, setAvatarError] = useState<string | null>(null);
  const [uploadingAvatar, setUploadingAvatar] = useState(false);

  const [followStats, setFollowStats] = useState<FollowStats | null>(null);

  const [exporting, setExporting] = useState(false);
  const [exportError, setExportError] = useState<string | null>(null);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deleteConfirmUsername, setDeleteConfirmUsername] = useState("");
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const [mfaMessage, setMfaMessage] = useState<string | null>(null);
  const [mfaError, setMfaError] = useState<string | null>(null);
  const [totpSetup, setTotpSetup] = useState<{
    secret: string;
    provisioning_uri: string;
    qr_code_data_uri: string;
  } | null>(null);
  const [totpCode, setTotpCode] = useState("");
  const [emailCode, setEmailCode] = useState("");
  const [emailSetupSent, setEmailSetupSent] = useState(false);

  const loadProfile = () => {
    if (!user) return;
    api.users
      .profile(user.username)
      .then((p) => {
        setProfile(p);
        setUsername(p.username);
        setBio(p.bio ?? "");
        setFirstName(p.first_name ?? "");
        setLastName(p.last_name ?? "");
        setDisplayName(p.display_name ?? "");
        setAuthorNameStyle(p.post_author_name_style);
        setCountry(p.country ?? "");
        setNativeLanguage(p.native_language);
        setFallbackLanguages(p.fallback_languages);
      })
      .catch((err) => setError(errorMessage(err)));
  };

  useEffect(loadProfile, [user]);

  useEffect(() => {
    authFetch((token) => api.users.followStats(token))
      .then(setFollowStats)
      .catch(() => undefined);
  }, [authFetch]);

  async function handleSaveBio(event: FormEvent) {
    event.preventDefault();
    setSavingBio(true);
    try {
      const updated = await authFetch((token) =>
        api.users.updateMe(token, {
          username,
          bio,
          first_name: firstName,
          last_name: lastName,
          display_name: displayName,
          post_author_name_style: authorNameStyle,
          country,
          native_language: nativeLanguage ?? "",
          fallback_languages: fallbackLanguages,
        })
      );
      setProfile(updated);
      // Lo username può essere cambiato: riallinea subito il resto della UI
      // (intestazione dashboard, autocomplete @menzioni, ecc.) che legge da qui.
      await refreshUser();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setSavingBio(false);
    }
  }

  async function handleAddLink(event: FormEvent) {
    event.preventDefault();
    setLinkError(null);
    try {
      await authFetch((token) => api.users.addSocialLink(token, { label: linkPlatform, url: linkUrl }));
      setLinkUrl("");
      loadProfile();
    } catch (err) {
      setLinkError(errorMessage(err));
    }
  }

  async function handleDeleteLink(linkId: string) {
    try {
      await authFetch((token) => api.users.deleteSocialLink(token, linkId));
      loadProfile();
    } catch (err) {
      setLinkError(errorMessage(err));
    }
  }

  async function handleAvatarChange(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    setAvatarError(null);
    setUploadingAvatar(true);
    try {
      await authFetch((token) => api.users.uploadAvatar(token, file));
      loadProfile();
    } catch (err) {
      setAvatarError(errorMessage(err));
    } finally {
      setUploadingAvatar(false);
      event.target.value = "";
    }
  }

  async function handleDeleteAvatar() {
    setAvatarError(null);
    try {
      await authFetch((token) => api.users.deleteAvatar(token));
      loadProfile();
    } catch (err) {
      setAvatarError(errorMessage(err));
    }
  }

  async function handleTotpSetup() {
    setMfaError(null);
    try {
      const res = await authFetch((token) => api.auth.totpSetup(token));
      setTotpSetup(res);
    } catch (err) {
      setMfaError(errorMessage(err));
    }
  }

  async function handleTotpConfirm(event: FormEvent) {
    event.preventDefault();
    setMfaError(null);
    try {
      await authFetch((token) => api.auth.totpConfirm(token, totpCode));
      setMfaMessage("Autenticazione a due fattori (app) attivata.");
      setTotpSetup(null);
      setTotpCode("");
    } catch (err) {
      setMfaError(errorMessage(err));
    }
  }

  async function handleEmailSetup() {
    setMfaError(null);
    try {
      await authFetch((token) => api.auth.emailSetup(token));
      setEmailSetupSent(true);
    } catch (err) {
      setMfaError(errorMessage(err));
    }
  }

  async function handleEmailConfirm(event: FormEvent) {
    event.preventDefault();
    setMfaError(null);
    try {
      await authFetch((token) => api.auth.emailConfirm(token, emailCode));
      setMfaMessage("Autenticazione a due fattori (email) attivata.");
      setEmailSetupSent(false);
      setEmailCode("");
    } catch (err) {
      setMfaError(errorMessage(err));
    }
  }

  async function handleDisableMfa() {
    setMfaError(null);
    try {
      await authFetch((token) => api.auth.disableMfa(token));
      setMfaMessage("Autenticazione a due fattori disattivata.");
    } catch (err) {
      setMfaError(errorMessage(err));
    }
  }

  async function handleExportData() {
    setExporting(true);
    setExportError(null);
    try {
      const data = await authFetch((token) => api.users.exportData(token));
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const link = document.createElement("a");
      link.href = url;
      link.download = `notturni-dati-${user?.username}.json`;
      link.click();
      URL.revokeObjectURL(url);
    } catch (err) {
      setExportError(errorMessage(err));
    } finally {
      setExporting(false);
    }
  }

  async function handleDeleteAccount(event: FormEvent) {
    event.preventDefault();
    setDeleting(true);
    setDeleteError(null);
    try {
      await authFetch((token) => api.users.deleteAccount(token, deleteConfirmUsername));
      await logout();
      router.push("/login");
    } catch (err) {
      setDeleteError(errorMessage(err));
      setDeleting(false);
    }
  }

  if (!user) return null;

  const authorNamePreview: Record<PostAuthorNameStyle, string> = {
    username: `@${username || user.username}`,
    full_name: [firstName, lastName].filter(Boolean).join(" ") || "Non impostato",
    display_name: displayName || "Non impostato",
  };

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-8 flex flex-col gap-1">
        <h1 className="font-serif text-[30px] font-medium leading-tight text-foreground">Profilo</h1>
        <p className="text-sm text-muted">Quello che i lettori vedono, quello che resta con te.</p>
      </div>
      {error && (
        <div className="mb-6">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      <div className="grid gap-10 lg:grid-cols-[180px_minmax(0,1fr)]">
        <nav className="hidden flex-col gap-0.5 text-sm text-muted lg:sticky lg:top-6 lg:flex lg:h-fit">
          <a href="#identita" className="rounded-md bg-primary/10 px-2.5 py-1.5 font-semibold text-foreground">
            Identità
          </a>
          <a href="#lingue" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            Lingue
          </a>
          <a href="#social" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            Link social
          </a>
          <a href="#sicurezza" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            Sicurezza · MFA
          </a>
          <a href="#privacy" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            Privacy e dati
          </a>
        </nav>

        <div className="flex flex-col gap-12">
          <form onSubmit={handleSaveBio} className="flex flex-col gap-12">
            <section id="identita" className="flex scroll-mt-6 flex-col gap-5">
              <h2 className="font-serif text-lg text-foreground">Identità</h2>

              <div className="grid gap-6 sm:grid-cols-[96px_minmax(0,1fr)] sm:items-start">
                <div className="flex flex-col items-center gap-2 sm:items-start">
                  {profile?.avatar_url ? (
                    <Image
                      src={profile.avatar_url}
                      alt={user.username}
                      width={96}
                      height={96}
                      className="h-24 w-24 rounded-full object-cover"
                      unoptimized
                    />
                  ) : (
                    <div className="flex h-24 w-24 items-center justify-center rounded-full bg-primary font-serif text-3xl text-background">
                      {user.username[0]?.toUpperCase()}
                    </div>
                  )}
                  <label className="cursor-pointer text-center text-[13px] text-primary hover:underline">
                    {uploadingAvatar ? "Caricamento…" : "Cambia"}
                    <input
                      type="file"
                      accept="image/png,image/jpeg,image/webp"
                      className="hidden"
                      disabled={uploadingAvatar}
                      onChange={handleAvatarChange}
                    />
                  </label>
                  {profile?.avatar_url && (
                    <button
                      type="button"
                      onClick={handleDeleteAvatar}
                      className="text-[13px] text-muted hover:text-foreground"
                    >
                      Rimuovi
                    </button>
                  )}
                  {avatarError && <p className="text-xs text-red-700">{avatarError}</p>}
                </div>

                <div className="grid gap-4 sm:grid-cols-2">
                  <FieldGroup className="mb-0">
                    <Label htmlFor="username" hint="referenziato per id — rinominalo liberamente">
                      Username
                    </Label>
                    <Input
                      id="username"
                      required
                      minLength={3}
                      maxLength={32}
                      value={username}
                      onChange={(e) => setUsername(e.target.value.toLowerCase())}
                    />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="display-name">Alias pubblico</Label>
                    <Input
                      id="display-name"
                      value={displayName}
                      maxLength={255}
                      placeholder="Lascia vuoto per usare lo username"
                      onChange={(e) => setDisplayName(e.target.value)}
                    />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="first-name">Nome</Label>
                    <Input id="first-name" value={firstName} onChange={(e) => setFirstName(e.target.value)} />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="last-name">Cognome</Label>
                    <Input id="last-name" value={lastName} onChange={(e) => setLastName(e.target.value)} />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="country">Paese</Label>
                    <Input
                      id="country"
                      maxLength={2}
                      placeholder="IT"
                      value={country}
                      onChange={(e) => setCountry(e.target.value.toUpperCase())}
                      className="w-20 uppercase"
                    />
                  </FieldGroup>
                </div>
              </div>
              <p className="-mt-2 text-xs text-muted">
                Citabile come @{username || "username"} nei post; cambiarlo si riflette subito su tutta la
                piattaforma (le @menzioni già scritte nel testo restano invariate).
              </p>

              <div className="flex flex-col gap-2">
                <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">
                  Firma i miei post come
                </span>
                <div className="grid gap-2.5 sm:grid-cols-3">
                  {(Object.keys(POST_AUTHOR_NAME_STYLE_LABELS) as PostAuthorNameStyle[]).map((s) => (
                    <button
                      key={s}
                      type="button"
                      onClick={() => setAuthorNameStyle(s)}
                      className={`flex flex-col gap-0.5 rounded-lg border px-3.5 py-3 text-left transition ${
                        authorNameStyle === s ? "border-primary bg-primary/5" : "border-border hover:border-primary/40"
                      }`}
                    >
                      <span className="text-sm font-semibold text-foreground">{POST_AUTHOR_NAME_STYLE_LABELS[s]}</span>
                      <span className="truncate text-[13px] text-muted">{authorNamePreview[s]}</span>
                    </button>
                  ))}
                </div>
                <span className="text-[13px] text-muted">
                  Un alias impostato dal blog (o dal tuo ruolo su quel blog) vince sempre su questa scelta.
                </span>
              </div>

              <FieldGroup className="mb-0">
                <Label htmlFor="bio">Bio</Label>
                <TextArea
                  id="bio"
                  value={bio}
                  onChange={(e) => setBio(e.target.value)}
                  placeholder="Racconta qualcosa di te…"
                />
              </FieldGroup>
            </section>

            <section id="lingue" className="flex scroll-mt-6 flex-col gap-5">
              <h2 className="font-serif text-lg text-foreground">Lingue</h2>
              <LanguagePicker
                nativeLanguage={nativeLanguage}
                onNativeLanguageChange={setNativeLanguage}
                fallbackLanguages={fallbackLanguages}
                onFallbackLanguagesChange={setFallbackLanguages}
              />
              <div>
                <Button type="submit" disabled={savingBio}>
                  {savingBio ? "Salvataggio…" : "Salva"}
                </Button>
              </div>
            </section>
          </form>

          {followStats && (
            <section className="flex flex-col gap-3">
              <h2 className="font-serif text-lg text-foreground">Follower</h2>
              <p className="text-sm text-foreground">
                <span className="font-medium">{followStats.total_followers}</span> in totale, sommando chi ti
                segue con il tuo username e chi segue i tuoi blog — anche quelli che si presentano con un alias
                diverso dal tuo nome.
              </p>
              <ul className="space-y-1 text-sm text-muted">
                <li>
                  @{profile?.username ?? username}: {followStats.user_followers}
                </li>
                {followStats.blogs.map((b) => (
                  <li key={b.blog_slug}>
                    {b.blog_title}
                    {b.alias && <span className="text-xs"> (alias: {b.alias})</span>}: {b.followers}
                  </li>
                ))}
              </ul>
              <p className="text-xs text-muted">
                Visibile solo a te: qui è l&apos;unico punto in cui username e alias dei blog vengono messi
                insieme. Ogni blog e il tuo profilo mostrano pubblicamente solo il proprio numero di follower,
                separatamente.
              </p>
            </section>
          )}

          <section id="social" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">Link social</h2>
            <div className="overflow-hidden rounded-lg border border-border">
              {(profile?.social_links.length ?? 0) === 0 && (
                <p className="px-4 py-3 text-sm text-muted">Nessun link aggiunto.</p>
              )}
              {profile?.social_links.map((link) => {
                const platform = getSocialPlatform(link.label);
                return (
                  <div
                    key={link.id}
                    className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5 text-sm last:border-0"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <platform.Icon className="shrink-0 text-foreground/70" />
                      <span className="text-foreground">{platform.label}</span>
                      <span className="truncate text-muted">{link.url}</span>
                    </span>
                    <button
                      type="button"
                      onClick={() => handleDeleteLink(link.id)}
                      className="shrink-0 text-muted hover:text-foreground"
                    >
                      Rimuovi
                    </button>
                  </div>
                );
              })}
            </div>
            {(profile?.social_links.length ?? 0) < 5 && (
              <form onSubmit={handleAddLink} className="flex flex-wrap items-end gap-3">
                <div>
                  <Label htmlFor="link-platform">Piattaforma</Label>
                  <select
                    id="link-platform"
                    value={linkPlatform}
                    onChange={(e) => setLinkPlatform(e.target.value)}
                    className="rounded-lg border border-border bg-surface px-3 py-2.5 text-sm text-foreground focus:outline-none focus:border-primary focus:ring-[3px] focus:ring-primary/20"
                  >
                    {SOCIAL_PLATFORMS.map((p) => (
                      <option key={p.key} value={p.key}>
                        {p.label}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="min-w-[200px] flex-1">
                  <Label htmlFor="link-url">URL</Label>
                  <Input
                    id="link-url"
                    required
                    type="url"
                    placeholder="https://…"
                    value={linkUrl}
                    onChange={(e) => setLinkUrl(e.target.value)}
                  />
                </div>
                <Button type="submit">Aggiungi</Button>
              </form>
            )}
            {linkError && <Alert kind="error">{linkError}</Alert>}
          </section>

          <section id="sicurezza" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">Sicurezza · MFA</h2>
            {user.mfa_enabled ? (
              <div className="flex flex-col gap-3">
                <Alert kind="success">Autenticazione a due fattori attiva.</Alert>
                <div>
                  <Button variant="secondary" onClick={handleDisableMfa}>
                    Disattiva
                  </Button>
                </div>
              </div>
            ) : (
              <div className="flex flex-col gap-6">
                <div>
                  <p className="mb-2 text-sm text-muted">App di autenticazione (TOTP)</p>
                  {!totpSetup ? (
                    <Button variant="secondary" onClick={handleTotpSetup}>
                      Configura
                    </Button>
                  ) : (
                    <form onSubmit={handleTotpConfirm} className="flex flex-col gap-3">
                      {/* eslint-disable-next-line @next/next/no-img-element -- SVG generato dal backend come data URI */}
                      <img
                        src={totpSetup.qr_code_data_uri}
                        alt="QR code per configurare l'app di autenticazione"
                        className="h-40 w-40 rounded-md border border-border bg-white p-2"
                      />
                      <details>
                        <summary className="cursor-pointer text-xs text-muted">
                          Non riesci a scansionare il QR? Inserisci il codice a mano
                        </summary>
                        <p className="mt-2 break-all rounded-md border border-border bg-foreground/5 p-3 font-mono text-xs">
                          {totpSetup.secret}
                        </p>
                      </details>
                      <p className="text-xs text-muted">
                        Inquadra il QR con la tua app di autenticazione, poi inserisci il codice generato.
                      </p>
                      <div className="flex items-end gap-3">
                        <Input
                          inputMode="numeric"
                          required
                          placeholder="123456"
                          value={totpCode}
                          onChange={(e) => setTotpCode(e.target.value)}
                        />
                        <Button type="submit">Conferma</Button>
                      </div>
                    </form>
                  )}
                </div>
                <div>
                  <p className="mb-2 text-sm text-muted">Codice via email</p>
                  {!emailSetupSent ? (
                    <Button variant="secondary" onClick={handleEmailSetup}>
                      Invia codice
                    </Button>
                  ) : (
                    <form onSubmit={handleEmailConfirm} className="flex items-end gap-3">
                      <Input
                        inputMode="numeric"
                        required
                        placeholder="123456"
                        value={emailCode}
                        onChange={(e) => setEmailCode(e.target.value)}
                      />
                      <Button type="submit">Conferma</Button>
                    </form>
                  )}
                </div>
              </div>
            )}
            {mfaMessage && <Alert kind="success">{mfaMessage}</Alert>}
            {mfaError && <Alert kind="error">{mfaError}</Alert>}
          </section>

          <section id="privacy" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">Privacy e dati</h2>
            <div className="overflow-hidden rounded-lg border border-border">
              <div className="flex items-center justify-between gap-5 border-b border-border px-4 py-4 last:border-0">
                <div className="flex flex-col gap-0.5">
                  <span className="text-sm font-medium text-foreground">Scarica i miei dati</span>
                  <span className="text-[13px] text-muted">
                    Profilo, blog di proprietà, post e commenti scritti, frammenti salvati, follow e token API.
                  </span>
                </div>
                <Button variant="secondary" size="sm" onClick={handleExportData} disabled={exporting}>
                  {exporting ? "Preparazione…" : "Scarica"}
                </Button>
              </div>
              <div className="flex items-center justify-between gap-5 px-4 py-4">
                <div className="flex flex-col gap-0.5">
                  <span className="text-sm font-medium text-foreground">Elimina il mio account</span>
                  <span className="text-[13px] text-muted">
                    Sessioni, token API, link social, frammenti e SSO rimossi per sempre. Blog, post e
                    commenti già scritti restano, con autore &quot;Utente eliminato&quot;. Non reversibile.
                  </span>
                </div>
                {!showDeleteConfirm && (
                  <Button variant="danger" size="sm" onClick={() => setShowDeleteConfirm(true)}>
                    Elimina
                  </Button>
                )}
              </div>
            </div>
            {exportError && <Alert kind="error">{exportError}</Alert>}
            {showDeleteConfirm && (
              <form onSubmit={handleDeleteAccount} className="flex flex-col gap-3 rounded-lg border border-danger/40 p-4">
                <FieldGroup className="mb-0">
                  <Label htmlFor="confirm-delete-username">
                    Per confermare, scrivi il tuo username (<strong>{user.username}</strong>)
                  </Label>
                  <Input
                    id="confirm-delete-username"
                    required
                    value={deleteConfirmUsername}
                    onChange={(e) => setDeleteConfirmUsername(e.target.value)}
                  />
                </FieldGroup>
                <div className="flex gap-3">
                  <Button type="submit" variant="danger" disabled={deleting}>
                    {deleting ? "Eliminazione…" : "Conferma eliminazione"}
                  </Button>
                  <Button
                    type="button"
                    variant="secondary"
                    onClick={() => {
                      setShowDeleteConfirm(false);
                      setDeleteConfirmUsername("");
                      setDeleteError(null);
                    }}
                  >
                    Annulla
                  </Button>
                </div>
                {deleteError && <Alert kind="error">{deleteError}</Alert>}
              </form>
            )}
            <p className="text-[13px] leading-relaxed text-muted">
              I dati sono trattati nell&apos;UE secondo il GDPR. La posizione usata per alba/tramonto del tema
              è calcolata sul tuo dispositivo e non ci viene mai inviata.
            </p>
          </section>
        </div>
      </div>
    </div>
  );
}
