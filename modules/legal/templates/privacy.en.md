*Last updated: {{last_update}}*

This policy explains how {{instance_fqdn}} ("the Service") processes the personal data of those who visit and use it, in accordance with Articles 13 and 14 of Regulation (EU) 2016/679 ("GDPR"). The Service is a publishing platform for texts (blogs, pages and newsletters) where each user decides what to make public.

## Data controller

- **{{controller_name}}**
- Address: {{controller_address}}
- Privacy contact: {{controller_email}}
{{#if controller_registry}}
- Registration details: {{controller_registry}}
{{/if}}
{{#if dpo_contact}}
- Data Protection Officer (DPO): {{dpo_contact}}
{{/if}}
{{#unless dpo_contact}}
- The Controller has not appointed a Data Protection Officer (DPO), as it is not required to.
{{/unless}}

## What this policy covers

It covers the data processed by the Service. It does not cover the sites reached through links or content embedded by authors. The texts, images and comments a user publishes are chosen by that user and are their responsibility: if they contain other people's data, the user must comply with the law. To report content that infringes your rights, write to the contact above{{#if reports}} or use the "Report" function available on content{{/if}}.

## What data we process, why and on what basis

**Account and sign-in.** Username, email address, role and interface language; the password is stored only as a hash (Argon2id) and cannot be read by anyone. They are used to create and manage your account and to let you sign in. Legal basis: performance of a contract (Art. 6(1)(b) GDPR).

**Profile and byline.** Display name, first and last name, biography, avatar, country, native language, interests and verified personal domain are all optional. The name you sign your texts with may differ from your real name: you can use an alias, even a different one for each blog. What you put in your public profile is visible to everyone; you can opt out of the public user directory in your profile settings. Legal basis: performance of a contract and, for optional fields, your consent (Art. 6(1)(a)).

**Content.** Posts, pages, notes, fragments, comments, images and other files you upload. We process them to publish them, show them to the readers you chose (public, members only or private) and store them. Legal basis: performance of a contract.

**Account security.** Sign-in sessions (we only keep a fingerprint of the session token and its expiry), API tokens (fingerprint and prefix only){{#if mfa}}, two-factor authentication (the TOTP secret is encrypted at rest; codes sent by email are temporary){{/if}}. Legal basis: performance of a contract and legitimate interest in security (Art. 6(1)(f) and Art. 32 GDPR).

**Activity log (audit log).** For security and accountability we record relevant actions (sign-ins, account changes, administration and moderation actions) with date, the person who performed them (username and email at the time), IP address and browser user agent. Legal basis: legitimate interest in protecting the Service and accounting for what happens (Art. 6(1)(f)).

**Abuse protection.** To limit repeated sign-in attempts and abnormal requests we temporarily keep in memory the IP address requests come from. Legal basis: legitimate interest (Art. 6(1)(f)).

**Service messages.**{{#if smtp}} We email you confirmations, sign-in codes, password resets, invitations to collaborate and notices about your account. These emails are necessary for the Service and are not promotional. Legal basis: performance of a contract.{{/if}}{{#unless smtp}} This instance is not configured to send email.{{/unless}}
{{#if sso}}

**Signing in with an external account.** If you choose to sign in with{{#if sso_google}} Google{{/if}}{{#if sso_microsoft}} Microsoft{{/if}}{{#if sso_github}} GitHub{{/if}}{{#if sso_linkedin}} LinkedIn{{/if}}, we receive from the provider and keep its identifier for your account and the associated email address. The provider processes your data as an independent controller under its own policy. Legal basis: performance of a contract.
{{/if}}
{{#if newsletter}}

**Newsletter.** If you subscribe to a newsletter we process your email address, language, subscription status and, as proof of consent, the date, IP address and user agent of the confirmation. Subscribing requires a confirmation email (double opt-in) and every message contains a link to unsubscribe and to delete your data without signing in. Legal basis: consent (Art. 6(1)(a)), which you can withdraw at any time.
{{/if}}
{{#if comments}}

**Comments.** Comments are visible according to the blog's settings. Registered users comment under their display name.{{#if turnstile}} If a blog's author allows comments from people without an account, we ask for the name and email you wish to give; those comments are moderated before publication and the form is protected by an anti-abuse check (see "Who we share data with").{{/if}} Legal basis: performance of a contract for registered users; legitimate interest in managing and moderating comments for others (Art. 6(1)(f)).
{{/if}}
{{#if discovery}}

**Reads and follows.** To show authors how many times a page was read we count reads as a number per post and per day, without cookies and without any identifier of the reader. If you follow a blog or a user we keep the link between the two accounts, which you need to receive your personal feed. Legal basis: performance of a contract and legitimate interest in providing aggregate statistics.
{{/if}}
{{#if reports}}

**Reports and moderation.** If you report content we keep the report, its author (the reporter) and the moderators' decisions. Legal basis: legitimate interest in keeping the Service safe and compliance with legal obligations on unlawful content (Art. 6(1)(c) and (f)).
{{/if}}
{{#if image_moderation}}

**Automatic image analysis.** Uploaded images are examined by software installed on the Service's own servers, without sending them to third parties, to recognise nudity and sensitive content and show them blurred with a notice. The outcome has no legal effect on you and is not profiling; you can contest it with a moderator. Legal basis: legitimate interest in protecting readers (Art. 6(1)(f)).
{{/if}}
{{#if custom_domains}}

**Personal domain.** If you connect your own domain we verify you own it with a DNS lookup and keep the domain and the outcome. Legal basis: performance of a contract.
{{/if}}
{{#if git_sync}}

**Git repository sync.** If you enable it, the Service sends the texts of your space to the repository you indicate and keeps, encrypted, the credential (token or deploy key) it needs. The repository is a service you chose and the data sent follows its rules; deleting a content item or your account here does not delete it there. Legal basis: performance of a contract at your request.
{{/if}}
{{#if link_preview}}

**Link previews.** When you paste an address into the editor, our server visits it to read the title and image and show a preview: the request comes from the Service's server, not from your device. Legal basis: performance of a contract.
{{/if}}

**Browsing data.** The servers technically log accesses (IP address, date and time, page requested, outcome) for operation and security; we do not use them to profile you. The Service uses no traffic analytics and no advertising. For cookies see the [Cookie policy](/p/cookie-policy).

Providing your account data is necessary to use the Service; the optional profile fields are not. Without account data we cannot give you access.

## Who we share data with

We do not sell data and do not use it for advertising or profiling. The following may access it:

- the Controller's authorised staff (administrators and moderators), only as far as their role requires; administrative access is logged;
- providers that process data on our behalf as processors (Art. 28 GDPR): {{hosting_description}}{{#if processors}}; {{processors}}{{/if}};
- authorities, where the law requires it.

**Other parties involved.**{{#if turnstile}} On pages of posts whose comments are open to people without an account, your browser loads an anti-abuse check from Cloudflare (Turnstile), which receives technical data about the device to tell people from automated programs.{{/if}}{{#if sso}} If you sign in with an external account, that provider receives the data needed for sign-in.{{/if}} Public content can be read by anyone, even without an account, and indexed by search engines according to the settings chosen by the blog's author.
{{#if transfers}}

## Transfers outside the European Economic Area

Some of the parties listed may process data outside the EEA. In that case the transfer relies on a European Commission adequacy decision or standard contractual clauses, with the safeguards of Arts. 44-49 GDPR. To obtain a copy of the safeguards write to the contact given in "Data controller".
{{/if}}

## How long we keep data

- **Account and content**: while the account is active. When you delete it the account is anonymised (see "Your rights").
- **Sign-in sessions**: they expire after {{session_days}} days.
- **Activity log**: {{audit_days}} days in the database.{{#if audit_archive}} Before deletion, events are copied to a protected archive, kept {{audit_archive_retention}}.{{/if}}
- **Newsletter subscriptions**: while you stay subscribed; you can delete your data at any time from the link in every email.
- **Deleted blogs**: recoverable for {{grace_days}} days, then permanently deleted.
{{#if backup}}
- **Backups**: {{backup_retention}}.
{{/if}}
- **GDPR requests**: handled within {{deletion_days}} days of receipt.

## Your rights

You have the right to access your data, rectify it, erase it, restrict its processing, receive it in a structured, machine-readable format (portability) and object to processing based on legitimate interest (Arts. 15-22 GDPR). Where processing is based on consent you can withdraw it at any time, without affecting processing already carried out.

{{#if gdpr_tools}}
From your dashboard you can **export all your data** as JSON and **delete your account**. For other requests write to {{controller_email}}.
{{/if}}
{{#unless gdpr_tools}}
To exercise your rights write to {{controller_email}}.
{{/unless}}
We reply within one month of the request (Art. 12 GDPR), extendable in complex cases.

**What "deleting the account" means.** The account is anonymised: username and email are replaced by anonymous values, sessions, tokens, follows and links to external providers are deleted, and signing in is no longer possible. The blogs, posts and comments you published remain visible under the author "Deleted user", because they also contain other people's work and rights (Art. 17(3) GDPR); if you want content removed, delete it first or ask us. The activity log is not deleted before the period stated in "How long we keep data", because it is needed for security and to account for the actions taken.

You also have the right to lodge a complaint with a supervisory authority, in particular in the Member State where you live or work. In Italy: the Garante per la protezione dei dati personali ([www.garanteprivacy.it](https://www.garanteprivacy.it)).

## Security

We adopt appropriate technical and organisational measures (Art. 32 GDPR), including encrypted connections, passwords stored only as hashes, encryption at rest of secrets, role-based access control, an activity log and rate limiting of sign-in attempts.

## Minors

The Service is not aimed at people under {{minimum_age|14}} and we do not knowingly collect data from minors of that age. If you believe a minor has given us data, write to us and we will delete it.

## Automated decisions

We do not take decisions based solely on automated processing that produce legal effects concerning you (Art. 22 GDPR).

## Changes

We may update this policy, for example when the Service's features change. The date at the top is the last revision; for significant changes we will notify you appropriately.
