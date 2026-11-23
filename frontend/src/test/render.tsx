import { render, type RenderOptions } from "@testing-library/react";
import { NextIntlClientProvider } from "next-intl";
import type { ReactElement } from "react";

import en from "@/messages/en.json";

/** Renders a component inside `NextIntlClientProvider` with the English messages. */
export function renderWithIntl(ui: ReactElement, options?: Omit<RenderOptions, "wrapper">) {
  return render(ui, {
    wrapper: ({ children }) => (
      <NextIntlClientProvider locale="en" messages={en} timeZone="Europe/Rome">
        {children}
      </NextIntlClientProvider>
    ),
    ...options,
  });
}
