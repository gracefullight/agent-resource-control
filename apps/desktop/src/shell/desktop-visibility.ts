import { isTauri } from "@tauri-apps/api/core";
import { TauriEvent } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export interface DesktopVisibility {
  subscribe(onVisible: () => void, onHidden: () => void): () => void;
}

export const desktopVisibility: DesktopVisibility = {
  subscribe(onVisible, onHidden) {
    let active = true;
    const unlisten: (() => void)[] = [];
    const accept = (cleanup: () => void) => {
      if (active) unlisten.push(cleanup);
      else cleanup();
    };
    const onDocumentVisibility = () => {
      if (document.hidden) onHidden();
      else onVisible();
    };
    document.addEventListener("visibilitychange", onDocumentVisibility);
    if (document.hidden) onHidden();
    if (isTauri()) {
      const window = getCurrentWindow();
      // Native lifecycle owns close prevention and hiding. These event-only
      // subscriptions retire UI work; they never add another close handler or
      // acquire daemon/process ownership. No extra window ACL is required.
      // Listen to the raw event. onCloseRequested() destroys the window after
      // its callback unless JS prevents it, which would duplicate native policy.
      void window
        .listen(TauriEvent.WINDOW_CLOSE_REQUESTED, () => {
          if (active) onHidden();
        })
        .then(accept, () => {});
      void window
        .onFocusChanged(({ payload: focused }) => {
          if (active && focused) onVisible();
        })
        .then(accept, () => {});
    }
    return () => {
      active = false;
      document.removeEventListener("visibilitychange", onDocumentVisibility);
      for (const cleanup of unlisten) cleanup();
    };
  },
};
