*Last updated: {{last_update}}*

This page explains which cookies and similar technologies {{instance_fqdn}} ("the Service") uses, under Article 5(3) of Directive 2002/58/EC and the guidance of the competent data protection authorities. For other personal data processed see the [Privacy policy](/p/privacy). The data controller is **{{controller_name}}** ({{controller_email}}).

## In short

The Service uses **technical cookies only**, necessary for it to work. It uses no profiling, advertising or traffic-analytics cookies, nor tools that follow you from site to site. For technical cookies the law does not require your consent, so you will not see a banner: the duty to inform you remains, and this page fulfils it.

## Cookies used

| Name | What it is for | Duration | Type |
|---|---|---|---|
| `noct_refresh_token` | Keeps your session after sign-in so you do not re-enter credentials on every page. Not readable by page scripts. | up to {{session_days}} days, or until sign-out | Technical, first party |
| `noct_csrf_token` | Protects requests that change data from forged requests sent by other sites. | up to {{session_days}} days, or until sign-out | Technical, first party |
{{#if sso}}
| `noct_sso_state` | Ties your browser to the request to sign in with an external account, so others cannot reuse it. | 10 minutes | Technical, first party |
{{/if}}
| `notturni_ui_locale` | Remembers the interface language you chose. | 1 year | Technical (preference), first party |

The sign-in cookies (`noct_refresh_token`, `noct_csrf_token`) are set only if you sign in; the language cookie only if you choose a language.

## Browser local storage

Besides cookies we use the browser's local storage, which stays on your device and is not sent to the Service:

- `notturni_theme_mode`: remembers whether you prefer the light, dark or automatic theme;
- `notturni_dashboard_sidebar_collapsed`: remembers whether you collapsed the side menu of your personal area.

## What we do not do

- No traffic analytics, no advertising, no pixels or device fingerprinting.
- Typefaces are served by the Service itself, not by external providers.
- Post reads are counted as a number per day, without cookies and without identifying the reader.

## Third-party services
{{#if turnstile}}

**Cloudflare Turnstile.** On pages of posts whose comments are open to people without an account, your browser loads from Cloudflare an anti-abuse check that tells people from automated programs. It may use on-device storage to work and is provided by Cloudflare, which is responsible for it; see its policy for details. It is not loaded anywhere else in the Service.
{{/if}}
{{#if sso}}

**Signing in with an external account.** If you choose to sign in with an external provider you are taken to its site, which may use its own cookies under its own policy.
{{/if}}

**Embedded content and links.** Authors may include images, links or content hosted elsewhere. When your browser loads them, the hosting site may receive technical data (for example your IP address) and set its own cookies: the Service has no control over this.

## How to manage or delete cookies

You can view, block or delete cookies from your browser settings. If you block technical cookies you will not be able to sign in to your personal area or keep a session; browsing public content keeps working. Deleting the sign-in cookies signs you out of the Service.

## Changes and contact

If the cookies used change we will update this page and, if needed, ask for your consent. For questions write to {{controller_email}}.
