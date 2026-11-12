import type { MeterSnapshot } from "@/bridge/meter.types";
import {
  mountDesktop,
  NativeFixture,
  settle,
  T,
  within,
} from "./desktop-test-harness";
import { meterFixture, meterReply } from "./meter-fixture";

export type View = ReturnType<typeof mountDesktop>;
export const metrics = (v: View) =>
  within(v.getByRole("region", { name: "Resource availability" }));
export const field = (v: View, label: string) => {
  const element = metrics(v).getByText(label).parentElement;
  if (!element) throw new Error(`Missing field ${label}`);
  return within(element);
};
export const text = (v: View, label: string) =>
  metrics(v).getByText(label).parentElement?.textContent ?? "";
export async function refresh(v: View, s?: MeterSnapshot, receipt = T) {
  if (s) v.native.meter = meterReply(s, receipt);
  await v.user.click(v.getByRole("button", { name: "Refresh status" }));
  await settle();
}
export async function full(s = meterFixture()) {
  const native = new NativeFixture();
  native.meter = meterReply(s);
  const v = mountDesktop(native);
  await settle();
  return v;
}
