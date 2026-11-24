/** Request headers returned by the mocked `next/headers` (see vitest.setup.ts). Reset after every test. */
const store = new Map<string, string>();

export const mockHeaders = {
  set(name: string, value: string) {
    store.set(name.toLowerCase(), value);
  },
  clear() {
    store.clear();
  },
  entries(): [string, string][] {
    return Array.from(store.entries());
  },
};
