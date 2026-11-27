import { SEARCH_MAX_LENGTH } from "@/lib/search";

interface SearchFormProps {
  defaultValue: string;
  placeholder: string;
  submitLabel: string;
}

/** Form di ricerca GET: funziona senza JavaScript lato client. Invia solo `q`, quindi una nuova
 * ricerca riparte dalla prima pagina. */
export function SearchForm({ defaultValue, placeholder, submitLabel }: SearchFormProps) {
  return (
    <form
      method="get"
      role="search"
      className="flex min-h-11 items-center gap-2 rounded-[10px] border border-border bg-surface py-1 pl-3.5 pr-1 text-sm text-muted md:w-96"
    >
      <span aria-hidden="true">⌕</span>
      <input
        // `defaultValue` si applica solo al montaggio: la key rimonta l'input quando la query cambia.
        key={defaultValue}
        name="q"
        type="text"
        defaultValue={defaultValue}
        maxLength={SEARCH_MAX_LENGTH}
        placeholder={placeholder}
        aria-label={placeholder}
        autoFocus
        className="min-w-0 flex-1 bg-transparent py-2 text-foreground outline-none placeholder:text-muted"
      />
      <button
        type="submit"
        className="flex h-11 shrink-0 items-center rounded-[8px] bg-primary px-4 font-medium text-background focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary"
      >
        {submitLabel}
      </button>
    </form>
  );
}
