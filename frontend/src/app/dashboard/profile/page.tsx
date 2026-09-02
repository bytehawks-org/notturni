"use client";

import Image from "next/image";
import { useEffect, useState, type FormEvent } from "react";

import { LanguagePicker } from "@/components/LanguagePicker";
import { Alert } from "@/components/ui/Alert";
import { Button } from "@/components/ui/Button";
import { Card, CardTitle } from "@/components/ui/Card";
import { FieldGroup, Input, Label, TextArea } from "@/components/ui/Field";
import { ApiClientError, api } from "@/lib/api";
import { useAuth } from "@/lib/auth-context";
import { SOCIAL_PLATFORMS, getSocialPlatform } from "@/lib/social-platforms";
import type { Profile } from "@/lib/types";

function errorMessage(err: unknown): string {
  return err instanceof ApiClientError ? err.message : "Errore imprevisto.";
}

export default function ProfilePage() {
  const { user, authFetch } = useAuth();
  const [profile, setProfile] = useState<Profile | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [bio, setBio] = useState("");
  const [firstName, setFirstName] = useState("");
  const [lastName, setLastName] = useState("");
  const [country, setCountry] = useState("");
  const [nativeLanguage, setNativeLanguage] = useState<string | null>(null);
  const [fallbackLanguages, setFallbackLanguages] = useState<string[]>([]);
  const [savingBio, setSavingBio] = useState(false);

  const [linkPlatform, setLinkPlatform] = useState(SOCIAL_PLATFORMS[0].key);
  const [linkUrl, setLinkUrl] = useState("");
  const [linkError, setLinkError] = useState<string | null>(null);

  const [avatarError, setAvatarError] = useState<string | null>(null);
  const [uploadingAvatar, setUploadingAvatar] = useState(false);

  const [mfaMessage, setMfaMessage] = useState<string | null>(null);
  const [mfaError, setMfaError] = useState<string | null>(null);
  const [totpSetup, setTotpSetup] = useState<{ secret: string; provisioning_uri: string } | null>(null);
  const [totpCode, setTotpCode] = useState("");
  const [emailCode, setEmailCode] = useState("");
  const [emailSetupSent, setEmailSetupSent] = useState(false);

  const loadProfile = () => {
    if (!user) return;
    api.users
      .profile(user.username)
      .then((p) => {
        setProfile(p);
        setBio(p.bio ?? "");
        setFirstName(p.first_name ?? "");
        setLastName(p.last_name ?? "");
        setCountry(p.country ?? "");
        setNativeLanguage(p.native_language);
        setFallbackLanguages(p.fallback_languages);
      })
      .catch((err) => setError(errorMessage(err)));
  };

  useEffect(loadProfile, [user]);

  async function handleSaveBio(event: FormEvent) {
    event.preventDefault();
    setSavingBio(true);
    try {
      const updated = await authFetch((token) =>
        api.users.updateMe(token, {
          bio,
          first_name: firstName,
          last_name: lastName,
          country,
          native_language: nativeLanguage ?? "",
          fallback_languages: fallbackLanguages,
        })
      );
      setProfile(updated);
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

  if (!user) return null;

  return (
    <div className="space-y-6">
      <h1 className="font-serif text-2xl text-foreground">Profilo</h1>
      {error && <Alert kind="error">{error}</Alert>}

      <Card>
        <CardTitle>Avatar</CardTitle>
        <div className="flex items-center gap-4">
          {profile?.avatar_url ? (
            <Image
              src={profile.avatar_url}
              alt={user.username}
              width={64}
              height={64}
              className="h-16 w-16 rounded-full object-cover"
              unoptimized
            />
          ) : (
            <div className="flex h-16 w-16 items-center justify-center rounded-full bg-foreground/10 text-lg text-muted">
              {user.username[0]?.toUpperCase()}
            </div>
          )}
          <div>
            <label className="inline-block cursor-pointer text-sm text-primary underline underline-offset-4">
              {uploadingAvatar ? "Caricamento…" : "Carica immagine"}
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
                className="ml-4 text-sm text-muted hover:text-foreground"
              >
                Rimuovi
              </button>
            )}
          </div>
        </div>
        {avatarError && (
          <div className="mt-3">
            <Alert kind="error">{avatarError}</Alert>
          </div>
        )}
      </Card>

      <Card>
        <CardTitle>Bio</CardTitle>
        <form onSubmit={handleSaveBio} className="space-y-4">
          <div className="flex flex-wrap gap-4">
            <FieldGroup>
              <Label htmlFor="first-name">Nome</Label>
              <Input id="first-name" value={firstName} onChange={(e) => setFirstName(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="last-name">Cognome</Label>
              <Input id="last-name" value={lastName} onChange={(e) => setLastName(e.target.value)} />
            </FieldGroup>
            <FieldGroup>
              <Label htmlFor="country">Paese (es. IT)</Label>
              <Input
                id="country"
                maxLength={2}
                value={country}
                onChange={(e) => setCountry(e.target.value.toUpperCase())}
                className="w-20 uppercase"
              />
            </FieldGroup>
          </div>

          <LanguagePicker
            nativeLanguage={nativeLanguage}
            onNativeLanguageChange={setNativeLanguage}
            fallbackLanguages={fallbackLanguages}
            onFallbackLanguagesChange={setFallbackLanguages}
          />

          <FieldGroup>
            <Label htmlFor="bio">Bio</Label>
            <TextArea
              id="bio"
              value={bio}
              onChange={(e) => setBio(e.target.value)}
              placeholder="Racconta qualcosa di te…"
            />
          </FieldGroup>
          <Button type="submit" disabled={savingBio}>
            {savingBio ? "Salvataggio…" : "Salva"}
          </Button>
        </form>
      </Card>

      <Card>
        <CardTitle>Link social</CardTitle>
        <ul className="mb-4 space-y-2">
          {profile?.social_links.map((link) => {
            const platform = getSocialPlatform(link.label);
            return (
              <li key={link.id} className="flex items-center justify-between text-sm">
                <span className="flex items-center gap-2">
                  <platform.Icon className="text-foreground/70" />
                  <span className="text-foreground">{platform.label}</span>{" "}
                  <span className="text-muted">{link.url}</span>
                </span>
                <button
                  type="button"
                  onClick={() => handleDeleteLink(link.id)}
                  className="text-muted hover:text-foreground"
                >
                  Rimuovi
                </button>
              </li>
            );
          })}
        </ul>
        {(profile?.social_links.length ?? 0) < 5 && (
          <form onSubmit={handleAddLink} className="flex flex-wrap items-end gap-3">
            <div>
              <Label htmlFor="link-platform">Piattaforma</Label>
              <select
                id="link-platform"
                value={linkPlatform}
                onChange={(e) => setLinkPlatform(e.target.value)}
                className="rounded-md border border-border bg-background px-3 py-2 text-sm text-foreground focus:outline-none focus:ring-2 focus:ring-primary/40"
              >
                {SOCIAL_PLATFORMS.map((p) => (
                  <option key={p.key} value={p.key}>
                    {p.label}
                  </option>
                ))}
              </select>
            </div>
            <div className="flex-1 min-w-[200px]">
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
        {linkError && (
          <div className="mt-3">
            <Alert kind="error">{linkError}</Alert>
          </div>
        )}
      </Card>

      <Card>
        <CardTitle>Autenticazione a due fattori</CardTitle>
        {user.mfa_enabled ? (
          <div className="space-y-3">
            <Alert kind="success">Attiva.</Alert>
            <Button variant="secondary" onClick={handleDisableMfa}>
              Disattiva
            </Button>
          </div>
        ) : (
          <div className="space-y-6">
            <div>
              <p className="mb-2 text-sm text-muted">App di autenticazione (TOTP)</p>
              {!totpSetup ? (
                <Button variant="secondary" onClick={handleTotpSetup}>
                  Configura
                </Button>
              ) : (
                <form onSubmit={handleTotpConfirm} className="space-y-3">
                  <p className="break-all rounded-md border border-border bg-foreground/5 p-3 font-mono text-xs">
                    {totpSetup.secret}
                  </p>
                  <p className="text-xs text-muted">
                    Aggiungi questo secret alla tua app di autenticazione, poi inserisci il codice
                    generato.
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
        {mfaMessage && (
          <div className="mt-3">
            <Alert kind="success">{mfaMessage}</Alert>
          </div>
        )}
        {mfaError && (
          <div className="mt-3">
            <Alert kind="error">{mfaError}</Alert>
          </div>
        )}
      </Card>
    </div>
  );
}
