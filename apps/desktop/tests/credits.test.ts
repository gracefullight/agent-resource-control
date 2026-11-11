import { afterEach, describe, expect, test } from "bun:test";
import {
  act,
  cleanup,
  clone,
  golden,
  mountDesktop,
  NativeFixture,
  settle,
  within,
} from "./fixtures/desktop-test-harness";

afterEach(cleanup);

async function openCredits(native = new NativeFixture()) {
  const view = mountDesktop(native);
  await settle();
  await view.user.click(view.getByRole("button", { name: "About / Credits" }));
  await settle();
  return view;
}

describe("production About / Credits", () => {
  test("exact SRS identity and three destinations use only the fixed native opener", async () => {
    const view = await openCredits();
    const credits = within(
      view.getByRole("region", { name: "Agent Resource Control" }),
    );
    expect(
      credits.getByRole("heading", { name: "Agent Resource Control" }),
    ).toBeDefined();
    expect(credits.getByText("Created by Eunkwang Shin")).toBeDefined();
    const github = credits.getByRole("link", {
      name: "Eunkwang Shin on GitHub",
    });
    const linkedin = credits.getByRole("link", {
      name: "Eunkwang Shin on LinkedIn",
    });
    const oma = credits.getByRole("link", { name: "Try OMA" });
    expect(github.getAttribute("href")).toBe(
      "https://github.com/gracefullight",
    );
    expect(linkedin.getAttribute("href")).toBe(
      "https://www.linkedin.com/in/gracefullight",
    );
    expect(oma.getAttribute("href")).toBe(
      "https://github.com/first-fluke/oh-my-agent",
    );
    for (const social of [github, linkedin]) {
      expect(social.textContent).toBe("");
      expect(social.querySelectorAll("svg")).toHaveLength(1);
      expect(social.querySelector("svg")?.getAttribute("aria-hidden")).toBe(
        "true",
      );
      expect(social.className).toContain("focus-visible:ring-2");
      expect(social.tabIndex).toBe(0);
      await view.user.click(social);
    }
    await act(async () => {
      oma.focus();
    });
    await view.user.keyboard("{Enter}");
    await settle();
    expect(
      view.native.calls
        .filter((call) => call.command === "open_credit_link")
        .map((call) => call.args),
    ).toEqual([{ link: "github" }, { link: "linkedin" }, { link: "oma" }]);
    expect(view.native.count("open_credit_link")).toBe(3);
  });

  test("social tooltips appear on keyboard focus and links participate in normal Tab order", async () => {
    const view = await openCredits();
    const github = view.getByRole("link", { name: "Eunkwang Shin on GitHub" });
    await view.user.tab();
    expect(document.activeElement === github).toBe(true);
    await settle();
    // Base UI 1.9 tooltips are supplementary visual labels, with the accessible
    // name on the trigger. Its popup intentionally has no tooltip ARIA role.
    expect(view.getByText("GitHub").hasAttribute("data-open")).toBe(true);
    await view.user.tab();
    expect(document.activeElement).toBe(
      view.getByRole("link", { name: "Eunkwang Shin on LinkedIn" }),
    );
    await settle();
    expect(view.getByText("LinkedIn").hasAttribute("data-open")).toBe(true);
    await view.user.tab();
    expect(document.activeElement).toBe(
      view.getByRole("link", { name: "Try OMA" }),
    );
  });

  test("null commit, dirty state and license remain unavailable while actual notices remain readable", async () => {
    const view = await openCredits();
    const credits = within(
      view.getByRole("region", { name: "Agent Resource Control" }),
    );
    expect(credits.getByText("Version").nextElementSibling?.textContent).toBe(
      "0.1.0",
    );
    expect(
      credits.getByText("OS architecture").nextElementSibling?.textContent,
    ).toBe("macos / aarch64");
    expect(
      credits.getByText("Build commit").nextElementSibling?.textContent,
    ).toBe("Unavailable");
    expect(
      credits.getByText("Build source state").nextElementSibling?.textContent,
    ).toBe("Unavailable");
    expect(
      credits.getByText("License unavailable in this build"),
    ).toBeDefined();
    expect(credits.queryByText("Apache-2.0")).toBeNull();
    const notices = credits.getByRole("button", {
      name: "Open-source notices",
    });
    await view.user.click(notices);
    expect(notices.getAttribute("aria-expanded")).toBe("true");
    expect(
      credits.getByRole("region", { name: "Open-source notices" }).textContent,
    ).toContain(golden.metadata.open_source_notices);
    await act(async () => {
      notices.focus();
    });
    await view.user.keyboard("{Enter}");
    expect(notices.getAttribute("aria-expanded")).toBe("false");
    await view.user.keyboard("{Enter}");
    expect(notices.getAttribute("aria-expanded")).toBe("true");
  });

  test("version, full build object, build state, architecture and notices come from local bridge metadata", async () => {
    const native = new NativeFixture();
    native.metadata = {
      ...clone(golden.metadata),
      version: "0.2.1-test",
      build_commit: "b".repeat(40),
      build_dirty: true,
      os: "macos",
      architecture: "x86_64",
      license: "Synthetic test license",
      open_source_notices:
        "Resolved test dependency\n<script>private marker</script>\nLicense test text",
    };
    const view = await openCredits(native);
    const credits = within(
      view.getByRole("region", { name: "Agent Resource Control" }),
    );
    expect(credits.getByText("0.2.1-test")).toBeDefined();
    expect(credits.getByText("b".repeat(40))).toBeDefined();
    expect(credits.getByText("Working tree contains changes")).toBeDefined();
    expect(credits.getByText("macos / x86_64")).toBeDefined();
    expect(credits.getByText("Synthetic test license")).toBeDefined();
    expect(view.container.querySelectorAll("script")).toHaveLength(0);
    expect(credits.getByText(/Resolved test dependency/).textContent).toContain(
      "<script>private marker</script>",
    );
    expect(native.count("desktop_metadata")).toBe(1);
  });

  test("missing notices and metadata failure are explicit and never depend on daemon connectivity", async () => {
    const native = new NativeFixture();
    native.healthFailure = { code: "unavailable", retryable: true };
    native.metadata = { ...clone(golden.metadata), open_source_notices: "" };
    const view = await openCredits(native);
    expect(
      view.getByText("Open-source notices unavailable in this build"),
    ).toBeDefined();
    expect(view.getByText("Version").nextElementSibling?.textContent).toBe(
      "0.1.0",
    );
    view.unmount();

    const failing = new NativeFixture();
    failing.enqueue("desktop_metadata", () =>
      Promise.reject("SECRET metadata diagnostic"),
    );
    const failedView = await openCredits(failing);
    expect(
      failedView.getByText("Desktop metadata is unavailable."),
    ).toBeDefined();
    expect(failedView.container.textContent).not.toContain("SECRET");
    expect(
      failedView.getByText("Build commit").nextElementSibling?.textContent,
    ).toBe("Unavailable");
  });

  test("opener failure is redacted, retains keyboard focus and permits another explicit activation", async () => {
    const native = new NativeFixture();
    native.enqueue("open_credit_link", () =>
      Promise.reject("SECRET /private/profile path"),
    );
    const view = await openCredits(native);
    const github = view.getByRole("link", { name: "Eunkwang Shin on GitHub" });
    await act(async () => {
      github.focus();
    });
    await view.user.keyboard("{Enter}");
    await settle();
    expect(
      view.getByText("The link could not be opened. Try again."),
    ).toBeDefined();
    expect(document.activeElement).toBe(github);
    expect(view.container.textContent).not.toContain("SECRET");
    await view.user.keyboard("{Enter}");
    await settle();
    expect(
      view.queryByText("The link could not be opened. Try again."),
    ).toBeNull();
    expect(native.count("open_credit_link")).toBe(2);
  });
});
