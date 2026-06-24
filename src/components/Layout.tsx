import { Link, Outlet } from "@tanstack/react-router";
import { Home, Layers, Settings } from "lucide-react";
import { cn } from "@/lib/utils";

const NAV = [
  { to: "/", label: "ホーム", icon: Home },
  { to: "/decks", label: "デッキ一覧", icon: Layers },
  { to: "/settings", label: "設定", icon: Settings },
] as const;

export function Layout() {
  return (
    <div className="flex h-screen w-screen overflow-hidden bg-background">
      {/* デスクトップ (md+): サイドバー */}
      <aside className="hidden w-56 shrink-0 flex-col border-r bg-card md:flex">
        <div className="flex h-14 items-center px-5 text-lg font-bold tracking-tight">
          単語帳
        </div>
        <nav className="flex flex-col gap-1 px-3 py-2">
          {NAV.map(({ to, label, icon: Icon }) => (
            <Link
              key={to}
              to={to}
              activeOptions={{ exact: to === "/" }}
              className="rounded-md"
              activeProps={{ className: "bg-accent text-accent-foreground" }}
            >
              {({ isActive }) => (
                <span
                  className={cn(
                    "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                    isActive
                      ? "bg-accent text-accent-foreground"
                      : "text-muted-foreground hover:bg-accent/50 hover:text-foreground",
                  )}
                >
                  <Icon className="h-4 w-4" />
                  {label}
                </span>
              )}
            </Link>
          ))}
        </nav>
      </aside>

      <div className="flex flex-1 flex-col overflow-hidden">
        {/* モバイル (< md): スリムなアプリ名ヘッダー */}
        <header className="flex h-12 shrink-0 items-center border-b bg-card px-4 text-base font-bold tracking-tight md:hidden">
          単語帳
        </header>
        <main className="flex-1 overflow-y-auto">
          {/* 下パディングは固定ボトムタブと重ならないための余白 (§6.2) */}
          <div className="mx-auto max-w-5xl p-4 pb-24 md:p-8 md:pb-8">
            <Outlet />
          </div>
        </main>
      </div>

      {/* モバイル (< md): 固定ボトムタブバー */}
      <nav
        className="fixed inset-x-0 bottom-0 z-20 flex border-t bg-card md:hidden"
        style={{ paddingBottom: "env(safe-area-inset-bottom)" }}
      >
        {NAV.map(({ to, label, icon: Icon }) => (
          <Link
            key={to}
            to={to}
            activeOptions={{ exact: to === "/" }}
            className="flex-1"
          >
            {({ isActive }) => (
              <span
                className={cn(
                  "flex h-16 flex-col items-center justify-center gap-1 text-xs font-medium transition-colors",
                  isActive
                    ? "text-accent-foreground"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                <Icon className={cn("h-5 w-5", isActive && "text-primary")} />
                {label}
              </span>
            )}
          </Link>
        ))}
      </nav>
    </div>
  );
}
