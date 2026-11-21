import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { listenForFront, refetchOnFront } from "./front";

describe("listenForFront", () => {
  it("tells of the page shown again and of the window getting the focus back", () => {
    const target = new EventTarget();
    const notify = vi.fn();
    listenForFront(target, notify);

    target.dispatchEvent(new Event("visibilitychange"));
    target.dispatchEvent(new Event("focus"));
    expect(notify).toHaveBeenCalledTimes(2);

    // Leaving the window is not coming back to it.
    target.dispatchEvent(new Event("blur"));
    expect(notify).toHaveBeenCalledTimes(2);
  });

  it("stops listening when told to", () => {
    const target = new EventTarget();
    const notify = vi.fn();
    const stop = listenForFront(target, notify);
    stop();
    target.dispatchEvent(new Event("visibilitychange"));
    target.dispatchEvent(new Event("focus"));
    expect(notify).not.toHaveBeenCalled();
  });
});

describe("refetchOnFront", () => {
  const stops: Array<() => void> = [];
  afterEach(() => {
    stops.splice(0).forEach((stop) => stop());
  });

  /** A mounted query (as a screen would have) counting how often it is loaded. */
  function watch(client: QueryClient, options: Partial<ConstructorParameters<typeof QueryObserver>[1]> = {}) {
    const loaded = { count: 0 };
    const observer = new QueryObserver(client, {
      queryKey: ["front", Math.random()],
      queryFn: () => ++loaded.count,
      ...options,
    });
    stops.push(observer.subscribe(() => {}));
    return loaded;
  }

  function client() {
    const c = new QueryClient({ defaultOptions: { queries: { retry: false, networkMode: "always" } } });
    c.mount();
    stops.push(() => c.unmount());
    return c;
  }

  it("loads the open screen's lists again when the window gets the focus back", async () => {
    const target = new EventTarget();
    refetchOnFront(target);
    const loaded = watch(client());
    await vi.waitFor(() => expect(loaded.count).toBe(1));

    target.dispatchEvent(new Event("focus"));
    await vi.waitFor(() => expect(loaded.count).toBe(2));

    target.dispatchEvent(new Event("visibilitychange"));
    await vi.waitFor(() => expect(loaded.count).toBe(3));
  });

  it("leaves the report being edited alone (useReportSource)", async () => {
    const target = new EventTarget();
    refetchOnFront(target);
    const c = client();
    const report = watch(c, { staleTime: Infinity, gcTime: 0, refetchOnWindowFocus: false });
    const list = watch(c);
    await vi.waitFor(() => expect(report.count + list.count).toBe(2));

    target.dispatchEvent(new Event("focus"));
    await vi.waitFor(() => expect(list.count).toBe(2));
    expect(report.count).toBe(1);
  });
});
