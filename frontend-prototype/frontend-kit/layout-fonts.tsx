// In src/app/layout.tsx replace the Inter/Lora imports with:
import { Lora, Source_Sans_3 } from "next/font/google";

export const bodyFont = Source_Sans_3({ variable: "--font-body", subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"] });
export const headingFont = Lora({ variable: "--font-heading", subsets: ["latin", "latin-ext"], weight: ["400", "500", "600"], style: ["normal", "italic"] });
// <html className={`${bodyFont.variable} ${headingFont.variable} h-full antialiased`}>

// layout.tsx — wrap the tree and set <html lang>:
//   import { NextIntlClientProvider } from "next-intl";
//   import { getLocale, getMessages } from "next-intl/server";
//   const locale = await getLocale(); const messages = await getMessages();
//   <html lang={locale} ...><body><NextIntlClientProvider locale={locale} messages={messages}><ThemeProvider>…</ThemeProvider></NextIntlClientProvider></body></html>
