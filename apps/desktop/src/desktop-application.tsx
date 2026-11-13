import { DesktopIcon, EyeSlashIcon, SignOutIcon } from "@phosphor-icons/react";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { desktopClient } from "@/bridge/client";
import type { DesktopClient } from "@/bridge/types";
import type { TimeFormat } from "@/components/recorded-time";
import { Button } from "@/components/ui/button";
import { DesktopCredits } from "@/credits/desktop-credits";
import { DesktopDashboard } from "@/dashboard/desktop-dashboard";
import {
  DesktopRefreshController,
  type RefreshClock,
  refreshClock,
} from "@/dashboard/desktop-refresh-controller";
import { desktopMessages as m } from "@/desktop-messages";
import {
  type DesktopVisibility,
  desktopVisibility,
} from "@/shell/desktop-visibility";

export interface DesktopApplicationProps extends TimeFormat {
  client?: DesktopClient;
  clock?: RefreshClock;
  visibility?: DesktopVisibility;
}
export function DesktopApplication({
  client = desktopClient,
  clock = refreshClock,
  visibility = desktopVisibility,
  ...format
}: DesktopApplicationProps) {
  const [controller] = useState(
    () => new DesktopRefreshController(client, clock),
  );
  const snapshot = useSyncExternalStore(
    controller.subscribe,
    controller.getSnapshot,
  );
  const [destination, setDestination] = useState<"dashboard" | "credits">(
    "dashboard",
  );
  const [shellPending, setShellPending] = useState(false);
  const [shellFailed, setShellFailed] = useState(false);
  const mounted = useRef(false);
  const shellIntent = useRef<"idle" | "action" | "quitting">("idle");

  useEffect(() => {
    mounted.current = true;
    controller.resume();
    const unsubscribe = visibility.subscribe(() => {
      if (shellIntent.current === "idle") controller.resume();
    }, controller.pause);
    return () => {
      unsubscribe();
      mounted.current = false;
      controller.pause();
    };
  }, [controller, visibility]);

  async function shellAction(action: "open" | "hide" | "quit") {
    if (shellIntent.current !== "idle") return;
    shellIntent.current = action === "quit" ? "quitting" : "action";
    setShellPending(true);
    setShellFailed(false);
    if (action !== "open") controller.pause();
    try {
      if (action === "open") {
        await client.openDashboard();
        if (!mounted.current) return;
        setDestination("dashboard");
        controller.resume();
      } else if (action === "hide") await client.hideDashboard();
      else await client.quitDesktop();
    } catch {
      shellIntent.current = "idle";
      if (mounted.current) {
        setShellFailed(true);
        controller.resume();
      }
    } finally {
      if (shellIntent.current !== "quitting") {
        shellIntent.current = "idle";
        if (mounted.current) setShellPending(false);
      }
    }
  }

  return (
    <main className="mx-auto flex min-h-screen w-full max-w-7xl flex-col gap-6 px-4 py-6 text-foreground sm:px-8">
      <header className="flex flex-wrap items-center justify-between gap-4 border-b border-border pb-5">
        <div className="flex min-w-0 items-center gap-3">
          <DesktopIcon aria-hidden="true" className="size-7 shrink-0" />
          <h1 className="text-lg font-semibold sm:text-xl">{m.productName}</h1>
        </div>
        <nav aria-label={m.navigation} className="flex flex-wrap gap-2">
          <Button
            variant={destination === "dashboard" ? "secondary" : "ghost"}
            aria-current={destination === "dashboard" ? "page" : undefined}
            className="h-auto min-h-9 max-w-full whitespace-normal"
            disabled={shellPending}
            onClick={() => void shellAction("open")}
          >
            {m.dashboard}
          </Button>
          <Button
            variant={destination === "credits" ? "secondary" : "ghost"}
            aria-current={destination === "credits" ? "page" : undefined}
            className="h-auto min-h-9 max-w-full whitespace-normal"
            onClick={() => setDestination("credits")}
          >
            {m.about}
          </Button>
        </nav>
      </header>
      {shellFailed && (
        <p role="status" className="text-sm text-destructive">
          {m.shellError}
        </p>
      )}
      <div hidden={destination !== "dashboard"}>
        <DesktopDashboard
          snapshot={snapshot}
          onRefresh={controller.refresh}
          onGovernorChange={controller.setGovernor}
          onSelect={controller.select}
          visible={destination === "dashboard" && snapshot.active}
          {...format}
        />
      </div>
      <div hidden={destination !== "credits"}>
        <DesktopCredits client={client} />
      </div>
      <footer className="mt-auto flex flex-wrap items-center justify-between gap-4 border-t border-border pt-5">
        <p className="max-w-xl text-xs text-muted-foreground">
          {m.persistenceHelp}
        </p>
        <div className="flex max-w-full flex-wrap gap-2">
          <Button
            variant="ghost"
            className="h-auto min-h-9 max-w-full whitespace-normal"
            disabled={shellPending}
            onClick={() => void shellAction("hide")}
          >
            <EyeSlashIcon aria-hidden="true" />
            {m.hide}
          </Button>
          <Button
            variant="outline"
            className="h-auto min-h-9 max-w-full whitespace-normal"
            disabled={shellPending}
            onClick={() => void shellAction("quit")}
          >
            <SignOutIcon aria-hidden="true" />
            {m.quit}
          </Button>
        </div>
      </footer>
    </main>
  );
}
