import Link from "next/link";

import { Card, CardTitle } from "@/components/ui/Card";

export default function AdminHomePage() {
  return (
    <div className="grid gap-4 sm:grid-cols-2">
      <Link href="/admin/pages">
        <Card className="h-full transition hover:border-primary">
          <CardTitle>Pagine statiche</CardTitle>
          <p className="text-sm text-muted">Chi siamo, Contatti, Privacy e traduzioni.</p>
        </Card>
      </Link>
      <Link href="/admin/users">
        <Card className="h-full transition hover:border-primary">
          <CardTitle>Utenti</CardTitle>
          <p className="text-sm text-muted">Ruoli e stato degli account della piattaforma.</p>
        </Card>
      </Link>
    </div>
  );
}
