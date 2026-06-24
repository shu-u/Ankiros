import { Moon, Sun } from "lucide-react";
import { useAppStore, type Theme } from "@/store/appStore";
import { logger } from "@/lib/logger";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

export function SettingsPage() {
  const theme = useAppStore((s) => s.theme);
  const setTheme = useAppStore((s) => s.setTheme);

  const options: { value: Theme; label: string; icon: typeof Sun }[] = [
    { value: "light", label: "ライト", icon: Sun },
    { value: "dark", label: "ダーク", icon: Moon },
  ];

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">設定</h1>
      <Card>
        <CardHeader>
          <CardTitle className="text-base">テーマ</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            {options.map(({ value, label, icon: Icon }) => (
              <Button
                key={value}
                variant={theme === value ? "default" : "outline"}
                onClick={() => {
                  void logger.debug(`Theme changed: ${value}`);
                  void setTheme(value);
                }}
                className={cn("gap-2")}
              >
                <Icon className="h-4 w-4" />
                {label}
              </Button>
            ))}
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
