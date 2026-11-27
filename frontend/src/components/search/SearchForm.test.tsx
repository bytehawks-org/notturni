import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SearchForm } from "@/components/search/SearchForm";

const form = (value: string) => <SearchForm defaultValue={value} placeholder="Search" submitLabel="Go" />;

describe("SearchForm", () => {
  it("shows the new query after a client navigation to another search", () => {
    const { rerender } = render(form("lanterna"));

    rerender(form("numero"));

    expect(screen.getByRole("textbox", { name: "Search" })).toHaveValue("numero");
  });
});
