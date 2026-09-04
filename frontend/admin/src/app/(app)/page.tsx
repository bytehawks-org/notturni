"use client";

import Link from "next/link";
import { useEffect, useState } from "react";

import { Card, CardTitle } from "@/components/ui/Card";
import { api } from "@/lib/api";

export default function AdminHomePage() {
  const [deploymentMode, setDeploymentMode] = useState<"solo" | "platform" | null>(null);

  useEffect(() => {
    api.config
      .get()
      .then((config) => setDeploymentMode(config.deployment_mode))
      .catch(() => setDeploymentMode("platform"));
  }, []);

  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <Link href="/pagine">
        <Card className="h-full transition hover:border-primary">
          <CardTitle>Pagine statiche</CardTitle>
          <p className="text-sm text-muted">Chi siamo, Contatti, Privacy e traduzioni.</p>
        </Card>
      </Link>
      {deploymentMode !== "solo" && (
        <Link href="/utenti">
          <Card className="h-full transition hover:border-primary">
            <CardTitle>Utenti</CardTitle>
            <p className="text-sm text-muted">Ruoli e stato degli account della piattaforma.</p>
          </Card>
        </Link>
      )}
      <Link href="/blog">
        <Card className="h-full transition hover:border-primary">
          <CardTitle>Blog</CardTitle>
          <p className="text-sm text-muted">Elenco dei blog, visibilità e sospensione.</p>
        </Card>
      </Link>
    </div>
  );
}
