import { Tooltip } from "@base-ui/react/tooltip";
import {
  ArrowSquareOutIcon,
  GithubLogoIcon,
  LinkedinLogoIcon,
} from "@phosphor-icons/react";
import { useEffect, useRef, useState } from "react";
import type {
  CreditLink,
  DesktopClient,
  DesktopMetadata,
} from "@/bridge/types";
import { Button } from "@/components/ui/button";
import { desktopMessages as m, message } from "@/desktop-messages";

export const CREDIT_DESTINATIONS: Record<CreditLink, string> = {
  github: "https://github.com/gracefullight",
  linkedin: "https://www.linkedin.com/in/gracefullight",
  oma: "https://github.com/first-fluke/oh-my-agent",
};
const creditFocus =
  "rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2";

interface DesktopCreditsProps {
  client: DesktopClient;
}
export function DesktopCredits({ client }: DesktopCreditsProps) {
  const [metadata, setMetadata] = useState<DesktopMetadata | null>(null);
  const [metadataState, setMetadataState] = useState<
    "loading" | "ready" | "error"
  >("loading");
  const [openerFailed, setOpenerFailed] = useState(false);
  const [noticesOpen, setNoticesOpen] = useState(false);
  const mounted = useRef(false);
  const openerGeneration = useRef(0);
  useEffect(() => {
    let active = true;
    mounted.current = true;
    void client.desktopMetadata().then(
      (value) => {
        if (active) {
          setMetadata(value);
          setMetadataState("ready");
        }
      },
      () => {
        if (active) setMetadataState("error");
      },
    );
    return () => {
      active = false;
      mounted.current = false;
      openerGeneration.current++;
    };
  }, [client]);

  async function open(link: CreditLink) {
    const generation = ++openerGeneration.current;
    setOpenerFailed(false);
    try {
      await client.openCreditLink(link);
    } catch {
      if (mounted.current && generation === openerGeneration.current)
        setOpenerFailed(true);
    }
  }

  return (
    <section
      aria-labelledby="credits-heading"
      className="mx-auto w-full max-w-3xl space-y-6 rounded-xl border border-border p-6 sm:p-8"
    >
      <header className="space-y-2">
        <h2 id="credits-heading" className="text-2xl font-semibold">
          {m.productName}
        </h2>
        <p>{m.creator}</p>
      </header>
      <Tooltip.Provider delay={200}>
        <div className="flex flex-wrap items-center gap-3">
          {(
            [
              ["github", m.githubName, m.githubTooltip, GithubLogoIcon],
              ["linkedin", m.linkedinName, m.linkedinTooltip, LinkedinLogoIcon],
            ] as const
          ).map(([link, name, tooltip, Icon]) => (
            <Tooltip.Root key={link}>
              <Tooltip.Trigger
                render={
                  <a
                    href={CREDIT_DESTINATIONS[link]}
                    aria-label={name}
                    onClick={(event) => {
                      event.preventDefault();
                      void open(link);
                    }}
                  />
                }
                className={`inline-flex size-10 items-center justify-center border border-border hover:bg-muted ${creditFocus}`}
              >
                <Icon aria-hidden="true" className="size-5" />
              </Tooltip.Trigger>
              <Tooltip.Portal>
                <Tooltip.Positioner sideOffset={6}>
                  <Tooltip.Popup className="rounded-md border border-border bg-foreground px-3 py-2 text-sm text-background shadow-md">
                    {tooltip}
                  </Tooltip.Popup>
                </Tooltip.Positioner>
              </Tooltip.Portal>
            </Tooltip.Root>
          ))}
          <a
            href={CREDIT_DESTINATIONS.oma}
            onClick={(event) => {
              event.preventDefault();
              void open("oma");
            }}
            className={`ml-auto inline-flex min-h-10 items-center gap-2 px-3 text-sm font-medium underline underline-offset-4 hover:bg-muted ${creditFocus}`}
          >
            {m.tryOma}
            <ArrowSquareOutIcon aria-hidden="true" className="size-4" />
          </a>
        </div>
      </Tooltip.Provider>
      {openerFailed && (
        <p role="status" className="text-sm text-destructive">
          {m.openerError}
        </p>
      )}
      {metadataState === "loading" && <p role="status">{m.metadataLoading}</p>}
      {metadataState === "error" && <p role="status">{m.metadataError}</p>}
      <dl className="grid min-w-0 gap-5 text-sm sm:grid-cols-2">
        <div>
          <dt className="text-muted-foreground">{m.version}</dt>
          <dd className="mt-1 break-words">
            {metadata?.version ?? m.metadataUnavailable}
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">{m.architecture}</dt>
          <dd className="mt-1 break-words">
            {metadata
              ? message(m.architectureValue, {
                  os: metadata.os,
                  architecture: metadata.architecture,
                })
              : m.metadataUnavailable}
          </dd>
        </div>
        <div className="sm:col-span-2">
          <dt className="text-muted-foreground">{m.buildCommit}</dt>
          <dd className="mt-1 break-all font-mono">
            {metadata?.build_commit ?? m.metadataUnavailable}
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">{m.buildState}</dt>
          <dd className="mt-1">
            {metadata?.build_dirty === true
              ? m.buildDirty
              : metadata?.build_dirty === false
                ? m.buildClean
                : m.metadataUnavailable}
          </dd>
        </div>
        <div>
          <dt className="text-muted-foreground">{m.license}</dt>
          <dd className="mt-1">{metadata?.license ?? m.licenseUnavailable}</dd>
        </div>
      </dl>
      <div className="border-t border-border pt-5">
        <Button
          variant="ghost"
          className="h-auto min-h-9 max-w-full whitespace-normal"
          aria-expanded={noticesOpen}
          aria-controls="desktop-notices"
          onClick={() => setNoticesOpen((open) => !open)}
        >
          {m.notices}
        </Button>
        <section
          id="desktop-notices"
          aria-label={m.notices}
          hidden={!noticesOpen}
        >
          {metadata?.open_source_notices.trim() ? (
            <pre className="mt-4 whitespace-pre-wrap break-words font-mono text-xs leading-relaxed">
              {metadata.open_source_notices}
            </pre>
          ) : (
            <p className="mt-3 text-sm text-muted-foreground">
              {m.noticesUnavailable}
            </p>
          )}
        </section>
      </div>
    </section>
  );
}
