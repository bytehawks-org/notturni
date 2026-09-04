import path from "node:path";
import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  output: "standalone",
  // frontend/ e frontend/admin/ sono due progetti Next.js indipendenti (non
  // un monorepo condiviso): senza questo, Turbopack risale fino al
  // package-lock.json di frontend/ e sceglie quella come workspace root.
  turbopack: {
    root: path.join(__dirname),
  },
};

export default nextConfig;
