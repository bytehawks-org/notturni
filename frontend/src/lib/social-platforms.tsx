import type { ComponentType, SVGProps } from "react";

/** Elenco delle piattaforme social disponibili nel profilo utente — un solo
 * posto da modificare per aggiungerne/toglierne una (icona monocromatica
 * inline, nessuna dipendenza esterna). Il valore salvato lato backend
 * (campo `label` di SocialLink, invariato per compatibilità) è la `key`
 * qui sotto, non più un'etichetta libera. Una `key` non presente in questo
 * elenco (link creati prima di questa modifica, o rimossi dalla lista) usa
 * comunque `GenericLinkIcon` — non è un errore, resta visualizzabile. */

function Icon(props: SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 18 18"
      width="16"
      height="16"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.4"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    />
  );
}

function MastodonIcon() {
  return (
    <Icon>
      <path d="M4 5.5c0-1.1 1.8-2 4.5-2s4.5.9 4.5 2v5c0 1.1-1.8 2-4.5 2-.8 0-1.5-.1-2.2-.2L5 15v-2.8C4.4 11.7 4 10.7 4 9.5v-4Z" />
      <path d="M7 6.5v3M11 6.5v3" />
    </Icon>
  );
}

function BlueskyIcon() {
  return (
    <Icon>
      <path d="M9 8c-.6-1.8-2.6-3.6-4.5-3.9-.3 2.6.4 5.4 2.3 6.7-1.9.2-3 1-3.3 2 1 .9 2.7 1 4-.1.6 1.1 1.5 1.9 1.5 1.9s.9-.8 1.5-1.9c1.3 1.1 3 1 4 .1-.3-1-1.4-1.8-3.3-2 1.9-1.3 2.6-4.1 2.3-6.7C11.6 4.4 9.6 6.2 9 8Z" />
    </Icon>
  );
}

function XIcon() {
  return (
    <Icon>
      <path d="M4 4l10 10M14 4L4 14" />
    </Icon>
  );
}

function GithubIcon() {
  return (
    <Icon>
      <path d="M9 3.5a5.5 5.5 0 0 0-1.7 10.7c.3 0 .4-.1.4-.3v-1.2c-1.5.3-1.9-.4-2-.7-.1-.2-.4-.7-.7-.8-.2-.1-.5-.4 0-.4.5 0 .8.5.9.6.5.9 1.4.6 1.7.5.1-.4.2-.6.4-.8-1.4-.2-2.9-.7-2.9-3 0-.7.2-1.2.6-1.7-.1-.2-.3-.8.1-1.6 0 0 .5-.2 1.7.6a5.7 5.7 0 0 1 3 0c1.2-.8 1.7-.6 1.7-.6.4.8.2 1.4.1 1.6.4.5.6 1 .6 1.7 0 2.3-1.5 2.8-2.9 3 .2.2.4.6.4 1.2v1.8c0 .2.1.3.4.3A5.5 5.5 0 0 0 9 3.5Z" />
    </Icon>
  );
}

function LinkedinIcon() {
  return (
    <Icon>
      <rect x="3" y="3" width="12" height="12" rx="2" />
      <path d="M6.3 7.5v4.2M6.3 5.8v.02M9 9v2.7M9 9c0-1 .7-1.5 1.5-1.5S12 8 12 9v2.7" />
    </Icon>
  );
}

function InstagramIcon() {
  return (
    <Icon>
      <rect x="3" y="3" width="12" height="12" rx="3.5" />
      <circle cx="9" cy="9" r="2.6" />
      <circle cx="12.3" cy="5.7" r="0.6" fill="currentColor" stroke="none" />
    </Icon>
  );
}

function YoutubeIcon() {
  return (
    <Icon>
      <rect x="2.5" y="4.5" width="13" height="9" rx="2.5" />
      <path d="M7.5 7v4l3.5-2-3.5-2Z" fill="currentColor" stroke="none" />
    </Icon>
  );
}

function WebsiteIcon() {
  return (
    <Icon>
      <circle cx="9" cy="9" r="6" />
      <path d="M3 9h12M9 3c1.8 1.7 2.8 4 2.8 6S10.8 13.3 9 15c-1.8-1.7-2.8-4-2.8-6S7.2 4.7 9 3Z" />
    </Icon>
  );
}

function EmailIcon() {
  return (
    <Icon>
      <rect x="2.5" y="4" width="13" height="10" rx="2" />
      <path d="M3 5.5 9 10l6-4.5" />
    </Icon>
  );
}

function RssIcon() {
  return (
    <Icon>
      <circle cx="4.5" cy="13.5" r="1.1" fill="currentColor" stroke="none" />
      <path d="M3.5 9a5.5 5.5 0 0 1 5.5 5.5M3.5 4.5A10 10 0 0 1 13.5 14.5" />
    </Icon>
  );
}

function GenericLinkIcon() {
  return (
    <Icon>
      <path d="M7.5 10.5a3 3 0 0 0 4.2.3l2-2a3 3 0 0 0-4.2-4.2l-1.1 1.1" />
      <path d="M10.5 7.5a3 3 0 0 0-4.2-.3l-2 2a3 3 0 0 0 4.2 4.2l1.1-1.1" />
    </Icon>
  );
}

export interface SocialPlatform {
  key: string;
  label: string;
  Icon: ComponentType<SVGProps<SVGSVGElement>>;
}

export const SOCIAL_PLATFORMS: SocialPlatform[] = [
  { key: "mastodon", label: "Mastodon", Icon: MastodonIcon },
  { key: "bluesky", label: "Bluesky", Icon: BlueskyIcon },
  { key: "x", label: "X (Twitter)", Icon: XIcon },
  { key: "github", label: "GitHub", Icon: GithubIcon },
  { key: "linkedin", label: "LinkedIn", Icon: LinkedinIcon },
  { key: "instagram", label: "Instagram", Icon: InstagramIcon },
  { key: "youtube", label: "YouTube", Icon: YoutubeIcon },
  { key: "website", label: "Sito web", Icon: WebsiteIcon },
  { key: "email", label: "Email", Icon: EmailIcon },
  { key: "rss", label: "RSS", Icon: RssIcon },
];

const BY_KEY = new Map(SOCIAL_PLATFORMS.map((p) => [p.key, p]));

export function getSocialPlatform(key: string): SocialPlatform {
  return BY_KEY.get(key) ?? { key, label: key, Icon: GenericLinkIcon };
}
