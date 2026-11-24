/** Generator for the first-run wizard: pure function, answers in, `.env` text and Helm `values.yaml` text out.
 * It prints placeholders only (`<generate>` ...), never real secrets, and changes nothing at runtime. */

export interface SetupAnswers {
  mode: "solo" | "platform";
  storage: "local" | "s3" | "garage";
  cache: "none" | "redis";
  smtp: "none" | "host" | "relay";
  subdomains: boolean;
  moderation: boolean;
  nats: boolean;
  /** Public host of the instance (`ingress.host`, `NOCT_INSTANCE_FQDN`). Empty = example.com placeholder. */
  host: string;
  /** Optional upstream smarthost for the relay branch. Emitted only when set. */
  relayHost: string;
}

export const SOLO_DEFAULTS: SetupAnswers = {
  mode: "solo",
  storage: "local",
  cache: "none",
  smtp: "none",
  subdomains: false,
  moderation: false,
  nats: false,
  host: "",
  relayHost: "",
};

export const PLATFORM_DEFAULTS: SetupAnswers = {
  mode: "platform",
  storage: "s3",
  cache: "redis",
  smtp: "host",
  subdomains: true,
  moderation: true,
  nats: true,
  host: "",
  relayHost: "",
};

/** Modules enabled without the optional `image_moderation` (same list as `.env.example`). */
export const BASE_MODULES = [
  "link_preview", "gdpr", "auth_mfa", "auth_sso", "pages", "backup", "comments", "discovery", "fragments",
  "git_sync", "graph", "interests", "legal", "newsletter", "notes", "publications", "reports", "taxonomy", "wiki",
  "branding", "custom_domains",
];

export function modulesFor(answers: SetupAnswers): string[] {
  return answers.moderation ? [...BASE_MODULES, "image_moderation"] : BASE_MODULES;
}

const hostOf = (a: SetupAnswers) => a.host.trim() || "example.com";

export function generateEnv(a: SetupAnswers): string {
  const host = hostOf(a);
  const l: string[] = [
    `NOCT_DEPLOYMENT_MODE=${a.mode}`,
    `NOCT_INSTANCE_FQDN=${host}`,
    `NOCT_SUBDOMAINS_ENABLED=${a.subdomains}`,
    `NOCT_MODULES=${modulesFor(a).join(",")}`,
  ];
  if (a.storage === "local") {
    l.push("NOCT_STORAGE_BACKEND=localstorage", `NOCT_LOCAL_STORAGE_PUBLIC_URL=https://${host}/storage`);
  } else if (a.storage === "garage") {
    l.push(
      "NOCT_STORAGE_BACKEND=s3",
      "NOCT_S3_ENDPOINT_URL=http://garage:3900",
      `NOCT_S3_PUBLIC_URL=https://s3.${host}`,
      "NOCT_S3_REGION=garage",
      "NOCT_S3_ACCESS_KEY_ID=<generate>",
      "NOCT_S3_SECRET_ACCESS_KEY=<generate>",
      "NOCT_GARAGE_RPC_SECRET=<generate>",
      "NOCT_GARAGE_ADMIN_TOKEN=<generate>",
    );
  } else {
    l.push(
      "NOCT_STORAGE_BACKEND=s3",
      "NOCT_S3_ENDPOINT_URL=<s3-endpoint-url>",
      "NOCT_S3_PUBLIC_URL=<s3-public-url>",
      "NOCT_S3_REGION=<region>",
      "NOCT_S3_ACCESS_KEY_ID=<generate>",
      "NOCT_S3_SECRET_ACCESS_KEY=<generate>",
    );
  }
  // The backend reads NOCT_REDIS_URL, or NOCT_REDIS_HOST/PORT/DB (the Helm chart sets the latter).
  l.push(a.cache === "redis" ? "NOCT_REDIS_URL=redis://redis:6379/0" : "# NOCT_REDIS_URL=  (in-memory cache)");
  if (a.nats) l.push("NOCT_NATS_URL=nats://nats:4222");
  else l.push("NOCT_NATS_URL=", "NOCT_WORKER_INLINE=true");
  if (a.smtp === "host") {
    l.push("NOCT_SMTP_HOST=<smtp-host>", "NOCT_SMTP_PORT=587", "NOCT_SMTP_USE_TLS=true", "NOCT_SMTP_USER=<user>", "NOCT_SMTP_PASSWORD=<generate>");
  } else if (a.smtp === "relay") {
    l.push("NOCT_SMTP_HOST=smtp-relay", "NOCT_SMTP_PORT=587", "NOCT_SMTP_USE_TLS=false");
    if (a.relayHost.trim()) l.push(`NOCT_SMTP_RELAY_HOST=${a.relayHost.trim()}`);
    l.push("NOCT_SMTP_RELAY_USERNAME=<user>", "NOCT_SMTP_RELAY_PASSWORD=<generate>");
  } else {
    l.push("NOCT_SMTP_HOST=");
  }
  l.push(a.moderation ? "NOCT_MODERATION_SERVICE_URL=http://moderation:8100" : "NOCT_MODERATION_SERVICE_URL=");
  return l.join("\n") + "\n";
}

/** Helm values matching `helm/notturni/values.yaml`. `modules` is never emitted: the chart default list is the
 * wizard list and `image_moderation` follows `moderation.enabled`. Credentials are never values: they go in the
 * Secret named by `secrets.existingSecret` (keys listed in the comments below). */
export function generateValues(a: SetupAnswers): string {
  const l: string[] = [`mode: ${a.mode}`, "", "ingress:", `  host: ${hostOf(a)}`];
  if (a.mode === "platform") l.push("", "admin:", "  email: admin@example.com");
  l.push("", "storage:", `  type: ${a.storage === "local" ? "local" : "s3"}`);
  if (a.storage === "s3") {
    l.push("  s3:", "    endpointUrl: <s3-endpoint-url>", "    publicUrl: <s3-public-url>", "    region: <region>");
  } else if (a.storage === "garage") {
    l.push("  s3:", "    garage:", "      enabled: true");
  }
  l.push("", "redis:", `  enabled: ${a.cache === "redis"}`, "", "nats:", `  enabled: ${a.nats}`);
  l.push("", "smtp:");
  if (a.smtp === "host") l.push("  host: <smtp-host>", "  port: 587", "  useTls: true", "  relay:", "    enabled: false");
  else if (a.smtp === "relay") {
    l.push("  relay:", "    enabled: true");
    if (a.relayHost.trim()) l.push(`    host: ${a.relayHost.trim()}`);
  } else l.push("  host: \"\"", "  relay:", "    enabled: false");
  l.push("", "moderation:", `  enabled: ${a.moderation}`, "", "subdomains:", `  enabled: ${a.subdomains}`);
  l.push("", "secrets:", "  existingSecret: <name-of-your-secret>");
  const keys = ["NOCT_POSTGRES_PASSWORD", "NOCT_JWT_SECRET", "NOCT_REVALIDATE_SECRET"];
  if (a.mode === "platform") keys.push("NOCT_SUPER_ADMIN_PASSWORD");
  if (a.storage === "s3") keys.push("NOCT_S3_ACCESS_KEY_ID", "NOCT_S3_SECRET_ACCESS_KEY");
  if (a.smtp === "host") keys.push("NOCT_SMTP_USER", "NOCT_SMTP_PASSWORD");
  if (a.smtp === "relay") keys.push("NOCT_SMTP_RELAY_USERNAME", "NOCT_SMTP_RELAY_PASSWORD");
  l.push("", "# Create the Secret above with these keys (never put them in this file):", ...keys.map((k) => `#   ${k}`));
  return l.join("\n") + "\n";
}
