"use client";

import { useTranslations } from "next-intl";
import Image from "next/image";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState, type FormEvent } from "react";

import { LanguagePicker } from "@/components/LanguagePicker";
import { UiLanguagePicker } from "@/components/shell/UiLanguagePicker";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { VerificationBadge } from "@/components/ui/VerificationBadge";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { SOCIAL_PLATFORMS, getSocialPlatform } from "@/lib/social-platforms";
import {
  type DomainOut,
  type FollowStats,
  type MeProfile,
  type PostAuthorNameStyle,
} from "@/lib/types";

const AUTHOR_NAME_STYLES: PostAuthorNameStyle[] = ["username", "full_name", "display_name", "verified_domain"];

export default function ProfilePage() {
  const router = useRouter();
  const { user, authFetch, refreshUser, logout } = useAuth();
  const t = useTranslations("Profile");
  const tc = useTranslations("Common");
  const tTier = useTranslations("VerificationTier");
  const errorMessage = useCallback(
    (err: unknown): string => (err instanceof ApiClientError ? err.message : tc("unexpectedError")),
    [tc]
  );
  const [profile, setProfile] = useState<MeProfile | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Aggiornato periodicamente (non catturato una sola volta al mount): il
  // cooldown username dura 5 giorni, una scheda lasciata aperta oltre quel
  // confine deve poter sbloccare da sola l'input invece di restarci
  // bloccata fino a un refresh manuale della pagina.
  const [nowMs, setNowMs] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNowMs(Date.now()), 60_000);
    return () => clearInterval(id);
  }, []);

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

  const [newEmail, setNewEmail] = useState("");
  const [emailChangeCode, setEmailChangeCode] = useState("");
  const [emailChangeError, setEmailChangeError] = useState<string | null>(null);
  const [emailChangeMessage, setEmailChangeMessage] = useState<string | null>(null);
  const [emailChangeSubmitting, setEmailChangeSubmitting] = useState(false);

  const [domainInput, setDomainInput] = useState("");
  const [domainInfo, setDomainInfo] = useState<DomainOut | null>(null);
  const [domainError, setDomainError] = useState<string | null>(null);
  const [domainMessage, setDomainMessage] = useState<string | null>(null);
  const [domainSubmitting, setDomainSubmitting] = useState(false);

  const loadProfile = useCallback(() => {
    if (!user) return;
    authFetch((token) => api.users.me(token))
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
        if (p.pending_email_change) {
          setNewEmail(p.pending_email_change.new_email);
        }
        if (p.domain_pending_verification && p.domain_verification_instructions) {
          setDomainInfo({
            domain: p.domain_pending_verification,
            status: "pending",
            txt_record_name: p.domain_verification_instructions.txt_record_name,
            txt_record_value: p.domain_verification_instructions.txt_record_value,
          });
        }
      })
      .catch((err) => setError(errorMessage(err)));
  }, [user, authFetch, errorMessage]);

  useEffect(loadProfile, [loadProfile]);

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

  async function handleRequestEmailChange(event: FormEvent) {
    event.preventDefault();
    setEmailChangeError(null);
    setEmailChangeMessage(null);
    setEmailChangeSubmitting(true);
    try {
      const res = await authFetch((token) => api.users.requestEmailChange(token, newEmail));
      setEmailChangeMessage(res.detail);
      loadProfile();
    } catch (err) {
      setEmailChangeError(errorMessage(err));
    } finally {
      setEmailChangeSubmitting(false);
    }
  }

  async function handleVerifyCurrentEmail(event: FormEvent) {
    event.preventDefault();
    setEmailChangeError(null);
    setEmailChangeSubmitting(true);
    try {
      const res = await authFetch((token) => api.users.verifyCurrentEmail(token, emailChangeCode));
      setEmailChangeMessage(res.detail);
      setEmailChangeCode("");
      loadProfile();
    } catch (err) {
      setEmailChangeError(errorMessage(err));
    } finally {
      setEmailChangeSubmitting(false);
    }
  }

  async function handleVerifyNewEmail(event: FormEvent) {
    event.preventDefault();
    setEmailChangeError(null);
    setEmailChangeSubmitting(true);
    try {
      const updated = await authFetch((token) => api.users.verifyNewEmail(token, emailChangeCode));
      setProfile(updated);
      setEmailChangeMessage(t("emailChanged"));
      setEmailChangeCode("");
      setNewEmail("");
    } catch (err) {
      setEmailChangeError(errorMessage(err));
    } finally {
      setEmailChangeSubmitting(false);
    }
  }

  async function handleCancelEmailChange() {
    setNewEmail("");
    setEmailChangeCode("");
    setEmailChangeError(null);
    setEmailChangeMessage(null);
    setProfile((prev) => (prev ? { ...prev, pending_email_change: null } : prev));
    try {
      // Senza questa chiamata la richiesta pending restava in DB (e il
      // vecchio OTP valido) anche dopo "Annulla" — sparivano solo lo stato
      // React locale, tornando visibili a un refresh della pagina.
      await authFetch((token) => api.users.cancelEmailChange(token));
    } catch {
      // Nessun riscontro bloccante qui: lo stato locale è già pulito e
      // request_email_change sostituisce comunque una pending precedente,
      // quindi un fallimento di rete su questa chiamata non lascia
      // l'interfaccia in uno stato inconsistente da segnalare.
    }
  }

  async function handleSaveDomain(event: FormEvent) {
    event.preventDefault();
    setDomainError(null);
    setDomainMessage(null);
    setDomainSubmitting(true);
    try {
      const res = await authFetch((token) => api.users.setDomain(token, domainInput));
      setDomainInfo(res);
      setDomainInput("");
    } catch (err) {
      setDomainError(errorMessage(err));
    } finally {
      setDomainSubmitting(false);
    }
  }

  async function handleVerifyDomain() {
    setDomainError(null);
    setDomainMessage(null);
    setDomainSubmitting(true);
    try {
      const res = await authFetch((token) => api.users.verifyDomain(token));
      setDomainInfo(res);
      if (res.status === "verified") {
        setDomainMessage(t("domainVerified"));
        loadProfile();
      }
    } catch (err) {
      setDomainError(errorMessage(err));
      // Il backend marca già il record "failed" in DB prima di rispondere
      // con l'errore (vedi app/api/v1/users.py::verify_my_domain) — senza
      // questo il badge di stato restava bloccato su "pending" fino a un
      // refresh, il ramo "failed" dell'interfaccia non si vedeva mai.
      setDomainInfo((prev) => (prev ? { ...prev, status: "failed" } : prev));
    } finally {
      setDomainSubmitting(false);
    }
  }

  async function handleRemoveDomain() {
    setDomainError(null);
    setDomainSubmitting(true);
    try {
      await authFetch((token) => api.users.deleteDomain(token));
      setDomainInfo(null);
      loadProfile();
    } catch (err) {
      setDomainError(errorMessage(err));
    } finally {
      setDomainSubmitting(false);
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
      setMfaMessage(t("mfaTotpOn"));
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
      setMfaMessage(t("mfaEmailOn"));
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
      setMfaMessage(t("mfaOff"));
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

  const usernameCooldownActive = Boolean(
    profile?.next_username_change_allowed_at &&
      new Date(profile.next_username_change_allowed_at).getTime() > nowMs
  );
  const usernameCooldownDateLabel = profile?.next_username_change_allowed_at
    ? new Date(profile.next_username_change_allowed_at).toLocaleDateString()
    : "";

  const authorNamePreview: Record<PostAuthorNameStyle, string> = {
    username: `@${username || user.username}`,
    full_name: [firstName, lastName].filter(Boolean).join(" ") || tc("notSet"),
    display_name: displayName || tc("notSet"),
    verified_domain: profile?.custom_domain ? `@${profile.custom_domain}` : tc("notSet"),
  };

  return (
    <div className="mx-auto max-w-5xl">
      <div className="mb-8 flex flex-col gap-1">
        <h1 className="font-serif text-[30px] font-medium leading-tight text-foreground">{t("title")}</h1>
        <p className="text-sm text-muted">{t("subtitle")}</p>
      </div>
      {error && (
        <div className="mb-6">
          <Alert kind="error">{error}</Alert>
        </div>
      )}

      <div className="grid gap-10 lg:grid-cols-[180px_minmax(0,1fr)]">
        <nav className="hidden flex-col gap-0.5 text-sm text-muted lg:sticky lg:top-6 lg:flex lg:h-fit">
          <a href="#identita" className="rounded-md bg-primary/10 px-2.5 py-1.5 font-semibold text-foreground">
            {t("nav.identity")}
          </a>
          <a href="#lingue" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            {t("nav.languages")}
          </a>
          <a href="#social" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            {t("nav.social")}
          </a>
          <a href="#email" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            {t("nav.email")}
          </a>
          <a href="#verifica" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            {t("nav.verification")}
          </a>
          <a href="#sicurezza" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            {t("nav.security")}
          </a>
          <a href="#privacy" className="rounded-md px-2.5 py-1.5 hover:text-foreground">
            {t("nav.privacy")}
          </a>
        </nav>

        <div className="flex flex-col gap-12">
          <form onSubmit={handleSaveBio} className="flex flex-col gap-12">
            <section id="identita" className="flex scroll-mt-6 flex-col gap-5">
              <h2 className="font-serif text-lg text-foreground">{t("nav.identity")}</h2>

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
                    {uploadingAvatar ? t("uploading") : t("changeAvatar")}
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
                      {tc("remove")}
                    </button>
                  )}
                  {avatarError && <p className="text-xs text-red-700">{avatarError}</p>}
                </div>

                <div className="grid gap-4 sm:grid-cols-2">
                  <FieldGroup className="mb-0">
                    <Label
                      htmlFor="username"
                      hint={
                        usernameCooldownActive
                          ? t("usernameCooldownHint", { date: usernameCooldownDateLabel })
                          : t("usernameHint")
                      }
                    >
                      {t("style.username")}
                    </Label>
                    <Input
                      id="username"
                      required
                      minLength={3}
                      maxLength={32}
                      value={username}
                      disabled={usernameCooldownActive}
                      onChange={(e) => setUsername(e.target.value.toLowerCase())}
                    />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="display-name">{t("displayName")}</Label>
                    <Input
                      id="display-name"
                      value={displayName}
                      maxLength={255}
                      placeholder={t("displayNamePlaceholder")}
                      onChange={(e) => setDisplayName(e.target.value)}
                    />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="first-name">{t("firstName")}</Label>
                    <Input id="first-name" value={firstName} onChange={(e) => setFirstName(e.target.value)} />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="last-name">{t("lastName")}</Label>
                    <Input id="last-name" value={lastName} onChange={(e) => setLastName(e.target.value)} />
                  </FieldGroup>
                  <FieldGroup className="mb-0">
                    <Label htmlFor="country">{t("country")}</Label>
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
              <p className="-mt-2 text-xs text-muted">{t("mentionNote", { username: username || "username" })}</p>

              <div className="flex flex-col gap-2">
                <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">
                  {t("signAs")}
                </span>
                <div className="grid gap-2.5 sm:grid-cols-4">
                  {AUTHOR_NAME_STYLES.map((s) => {
                    const unavailable = s === "verified_domain" && !profile?.custom_domain;
                    return (
                      <button
                        key={s}
                        type="button"
                        disabled={unavailable}
                        onClick={() => setAuthorNameStyle(s)}
                        title={unavailable ? t("style.verifiedDomainUnavailable") : undefined}
                        className={`flex flex-col gap-0.5 rounded-lg border px-3.5 py-3 text-left transition ${
                          unavailable
                            ? "cursor-not-allowed border-border opacity-50"
                            : authorNameStyle === s
                              ? "border-primary bg-primary/5"
                              : "border-border hover:border-primary/40"
                        }`}
                      >
                        <span className="text-sm font-semibold text-foreground">{t(`style.${s}`)}</span>
                        <span className="truncate text-[13px] text-muted">{authorNamePreview[s]}</span>
                      </button>
                    );
                  })}
                </div>
                <span className="text-[13px] text-muted">{t("signAsNote")}</span>
              </div>

              <FieldGroup className="mb-0">
                <Label htmlFor="bio">{t("bio")}</Label>
                <TextArea
                  id="bio"
                  value={bio}
                  onChange={(e) => setBio(e.target.value)}
                  placeholder={t("bioPlaceholder")}
                />
              </FieldGroup>
            </section>

            <section id="lingue" className="flex scroll-mt-6 flex-col gap-5">
              <h2 className="font-serif text-lg text-foreground">{t("nav.languages")}</h2>
              <LanguagePicker
                nativeLanguage={nativeLanguage}
                onNativeLanguageChange={setNativeLanguage}
                fallbackLanguages={fallbackLanguages}
                onFallbackLanguagesChange={setFallbackLanguages}
              />
              <div className="flex flex-col gap-1.5">
                <span className="text-xs font-semibold uppercase tracking-[.04em] text-muted">{t("interfaceLanguage")}</span>
                <UiLanguagePicker className="w-fit" />
                <span className="text-[13px] text-muted">{t("interfaceLanguageNote")}</span>
              </div>
              <div>
                <Button type="submit" disabled={savingBio}>
                  {savingBio ? tc("saving") : tc("save")}
                </Button>
              </div>
            </section>
          </form>

          {followStats && (
            <section className="flex flex-col gap-3">
              <h2 className="font-serif text-lg text-foreground">{t("followers")}</h2>
              <p className="text-sm text-foreground">
                {t.rich("followersTotal", {
                  count: followStats.total_followers,
                  b: (chunks) => <span className="font-medium">{chunks}</span>,
                })}
              </p>
              <ul className="space-y-1 text-sm text-muted">
                <li>
                  @{profile?.username ?? username}: {followStats.user_followers}
                </li>
                {followStats.blogs.map((b) => (
                  <li key={b.blog_slug}>
                    {b.blog_title}
                    {b.alias && <span className="text-xs"> ({t("alias", { alias: b.alias })})</span>}: {b.followers}
                  </li>
                ))}
              </ul>
              <p className="text-xs text-muted">{t("followersPrivacy")}</p>
            </section>
          )}

          <section id="social" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">{t("nav.social")}</h2>
            <div className="overflow-hidden rounded-lg border border-border">
              {(profile?.social_links.length ?? 0) === 0 && (
                <p className="px-4 py-3 text-sm text-muted">{t("noLinks")}</p>
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
                      {tc("remove")}
                    </button>
                  </div>
                );
              })}
            </div>
            {(profile?.social_links.length ?? 0) < 5 && (
              <form onSubmit={handleAddLink} className="flex flex-wrap items-end gap-3">
                <div>
                  <Label htmlFor="link-platform">{t("platform")}</Label>
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
                  <Label htmlFor="link-url">{t("url")}</Label>
                  <Input
                    id="link-url"
                    required
                    type="url"
                    placeholder="https://…"
                    value={linkUrl}
                    onChange={(e) => setLinkUrl(e.target.value)}
                  />
                </div>
                <Button type="submit">{tc("add")}</Button>
              </form>
            )}
            {linkError && <Alert kind="error">{linkError}</Alert>}
          </section>

          <section id="email" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">{t("nav.email")}</h2>
            <p className="text-sm text-foreground">
              {t("emailCurrent")}: <span className="font-medium">{profile?.email}</span>
            </p>

            {!profile?.pending_email_change && (
              <form onSubmit={handleRequestEmailChange} className="flex flex-wrap items-end gap-3">
                <div className="min-w-[240px] flex-1">
                  <Label htmlFor="new-email">{t("emailNewLabel")}</Label>
                  <Input
                    id="new-email"
                    type="email"
                    required
                    value={newEmail}
                    onChange={(e) => setNewEmail(e.target.value)}
                  />
                </div>
                <Button type="submit" variant="secondary" disabled={emailChangeSubmitting}>
                  {t("emailSendCode")}
                </Button>
              </form>
            )}

            {profile?.pending_email_change?.stage === "awaiting_old_confirmation" && (
              <form onSubmit={handleVerifyCurrentEmail} className="flex flex-col gap-3">
                <p className="text-[13px] text-muted">
                  {t("emailStepOldHint", { email: profile.email })}
                </p>
                <div className="flex items-end gap-3">
                  <Input
                    inputMode="numeric"
                    required
                    placeholder="123456"
                    value={emailChangeCode}
                    onChange={(e) => setEmailChangeCode(e.target.value)}
                  />
                  <Button type="submit" disabled={emailChangeSubmitting}>
                    {t("emailConfirm")}
                  </Button>
                  <Button type="button" variant="secondary" onClick={handleCancelEmailChange}>
                    {t("emailCancel")}
                  </Button>
                </div>
              </form>
            )}

            {profile?.pending_email_change?.stage === "awaiting_new_confirmation" && (
              <form onSubmit={handleVerifyNewEmail} className="flex flex-col gap-3">
                <p className="text-[13px] text-muted">
                  {t("emailStepNewHint", { email: profile.pending_email_change.new_email })}
                </p>
                <div className="flex items-end gap-3">
                  <Input
                    inputMode="numeric"
                    required
                    placeholder="123456"
                    value={emailChangeCode}
                    onChange={(e) => setEmailChangeCode(e.target.value)}
                  />
                  <Button type="submit" disabled={emailChangeSubmitting}>
                    {t("emailConfirm")}
                  </Button>
                  <Button type="button" variant="secondary" onClick={handleCancelEmailChange}>
                    {t("emailCancel")}
                  </Button>
                </div>
              </form>
            )}

            {emailChangeMessage && <Alert kind="success">{emailChangeMessage}</Alert>}
            {emailChangeError && <Alert kind="error">{emailChangeError}</Alert>}
          </section>

          <section id="verifica" className="flex scroll-mt-6 flex-col gap-6">
            <h2 className="font-serif text-lg text-foreground">{t("nav.verification")}</h2>

            <div className="flex items-center gap-2">
              <VerificationBadge
                tier={profile?.verification_tier ?? "none"}
                size={20}
                label={profile && profile.verification_tier !== "none" ? tTier(profile.verification_tier) : undefined}
              />
              <span className="text-sm text-foreground">
                {profile && profile.verification_tier !== "none"
                  ? t("verificationBadgeActive")
                  : t("verificationBadgeNone")}
              </span>
            </div>

            <div className="flex flex-col gap-3">
              <div>
                <h3 className="text-sm font-semibold text-foreground">{t("domainTitle")}</h3>
                <p className="text-[13px] text-muted">{t("domainSub")}</p>
              </div>

              {profile?.custom_domain && (
                <Alert kind="success">
                  {profile.custom_domain} — {t("domainVerified")}
                </Alert>
              )}

              {!profile?.custom_domain && domainInfo && (
                <div className="flex flex-col gap-2 rounded-lg border border-border p-4">
                  <p className="text-sm text-foreground">
                    {domainInfo.domain} —{" "}
                    {domainInfo.status === "failed" ? t("domainStatusFailed") : t("domainStatusPending")}
                  </p>
                  <p className="text-[13px] text-muted">{t("domainInstructions")}</p>
                  <div className="rounded-md border border-border bg-foreground/5 p-3 font-mono text-xs">
                    <p>{domainInfo.txt_record_name}</p>
                    <p className="break-all">{domainInfo.txt_record_value}</p>
                  </div>
                  <div className="flex gap-3">
                    <Button
                      type="button"
                      variant="secondary"
                      size="sm"
                      disabled={domainSubmitting}
                      onClick={handleVerifyDomain}
                    >
                      {t("domainVerify")}
                    </Button>
                  </div>
                </div>
              )}

              {!domainInfo && !profile?.custom_domain && (
                <form onSubmit={handleSaveDomain} className="flex flex-wrap items-end gap-3">
                  <div className="min-w-[240px] flex-1">
                    <Label htmlFor="custom-domain">{t("domainLabel")}</Label>
                    <Input
                      id="custom-domain"
                      required
                      placeholder={t("domainPlaceholder")}
                      value={domainInput}
                      onChange={(e) => setDomainInput(e.target.value)}
                    />
                  </div>
                  <Button type="submit" variant="secondary" disabled={domainSubmitting}>
                    {t("domainSave")}
                  </Button>
                </form>
              )}

              {(domainInfo || profile?.custom_domain) && (
                <div>
                  <Button type="button" variant="secondary" size="sm" onClick={handleRemoveDomain}>
                    {t("domainRemove")}
                  </Button>
                </div>
              )}

              {domainMessage && <Alert kind="success">{domainMessage}</Alert>}
              {domainError && <Alert kind="error">{domainError}</Alert>}
            </div>

            <div className="flex flex-col gap-2">
              <div>
                <h3 className="text-sm font-semibold text-foreground">{t("fediverseTitle")}</h3>
                <p className="text-[13px] text-muted">{t("fediverseNote")}</p>
              </div>
              <div className="flex flex-col gap-1.5 text-sm">
                <div>
                  <span className="text-muted">{t("fediverseAtproto")}: </span>
                  <span className="break-all font-mono text-xs text-foreground">{profile?.atproto_did}</span>
                </div>
                <div>
                  <span className="text-muted">{t("fediverseActivitypub")}: </span>
                  <span className="break-all font-mono text-xs text-foreground">
                    {profile?.activitypub_actor_id}
                  </span>
                </div>
              </div>
            </div>
          </section>

          <section id="sicurezza" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">{t("nav.security")}</h2>
            {user.mfa_enabled ? (
              <div className="flex flex-col gap-3">
                <Alert kind="success">{t("mfaActive")}</Alert>
                <div>
                  <Button variant="secondary" onClick={handleDisableMfa}>
                    {t("disable")}
                  </Button>
                </div>
              </div>
            ) : (
              <div className="flex flex-col gap-6">
                <div>
                  <p className="mb-2 text-sm text-muted">{t("totp")}</p>
                  {!totpSetup ? (
                    <Button variant="secondary" onClick={handleTotpSetup}>
                      {t("configure")}
                    </Button>
                  ) : (
                    <form onSubmit={handleTotpConfirm} className="flex flex-col gap-3">
                      {/* eslint-disable-next-line @next/next/no-img-element -- SVG generato dal backend come data URI */}
                      <img
                        src={totpSetup.qr_code_data_uri}
                        alt={t("qrAlt")}
                        className="h-40 w-40 rounded-md border border-border bg-white p-2"
                      />
                      <details>
                        <summary className="cursor-pointer text-xs text-muted">{t("qrManual")}</summary>
                        <p className="mt-2 break-all rounded-md border border-border bg-foreground/5 p-3 font-mono text-xs">
                          {totpSetup.secret}
                        </p>
                      </details>
                      <p className="text-xs text-muted">{t("qrHint")}</p>
                      <div className="flex items-end gap-3">
                        <Input
                          inputMode="numeric"
                          required
                          placeholder="123456"
                          value={totpCode}
                          onChange={(e) => setTotpCode(e.target.value)}
                        />
                        <Button type="submit">{tc("confirm")}</Button>
                      </div>
                    </form>
                  )}
                </div>
                <div>
                  <p className="mb-2 text-sm text-muted">{t("emailCode")}</p>
                  {!emailSetupSent ? (
                    <Button variant="secondary" onClick={handleEmailSetup}>
                      {t("sendCode")}
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
                      <Button type="submit">{tc("confirm")}</Button>
                    </form>
                  )}
                </div>
              </div>
            )}
            {mfaMessage && <Alert kind="success">{mfaMessage}</Alert>}
            {mfaError && <Alert kind="error">{mfaError}</Alert>}
          </section>

          <section id="privacy" className="flex scroll-mt-6 flex-col gap-4">
            <h2 className="font-serif text-lg text-foreground">{t("nav.privacy")}</h2>
            <div className="overflow-hidden rounded-lg border border-border">
              <div className="flex items-center justify-between gap-5 border-b border-border px-4 py-4 last:border-0">
                <div className="flex flex-col gap-0.5">
                  <span className="text-sm font-medium text-foreground">{t("downloadData")}</span>
                  <span className="text-[13px] text-muted">{t("downloadDataSub")}</span>
                </div>
                <Button variant="secondary" size="sm" onClick={handleExportData} disabled={exporting}>
                  {exporting ? t("preparing") : t("download")}
                </Button>
              </div>
              <div className="flex items-center justify-between gap-5 px-4 py-4">
                <div className="flex flex-col gap-0.5">
                  <span className="text-sm font-medium text-foreground">{t("deleteAccount")}</span>
                  <span className="text-[13px] text-muted">{t("deleteAccountSub")}</span>
                </div>
                {!showDeleteConfirm && (
                  <Button variant="danger" size="sm" onClick={() => setShowDeleteConfirm(true)}>
                    {tc("delete")}
                  </Button>
                )}
              </div>
            </div>
            {exportError && <Alert kind="error">{exportError}</Alert>}
            {showDeleteConfirm && (
              <form onSubmit={handleDeleteAccount} className="flex flex-col gap-3 rounded-lg border border-danger/40 p-4">
                <FieldGroup className="mb-0">
                  <Label htmlFor="confirm-delete-username">
                    {t.rich("confirmDeleteLabel", { username: user.username, b: (chunks) => <strong>{chunks}</strong> })}
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
                    {deleting ? t("deleting") : t("confirmDelete")}
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
                    {tc("cancel")}
                  </Button>
                </div>
                {deleteError && <Alert kind="error">{deleteError}</Alert>}
              </form>
            )}
            <p className="text-[13px] leading-relaxed text-muted">{t("gdprNote")}</p>
          </section>
        </div>
      </div>
    </div>
  );
}
