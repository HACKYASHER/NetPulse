import type { ClientRelease } from "./types";
export const INITIAL_UPDATE_DELAY = 5_000;
export const UPDATE_CHECK_INTERVAL = 6 * 60 * 60 * 1_000;
type Timers = {
  after: (callback: () => void, ms: number) => unknown;
  every: (callback: () => void, ms: number) => unknown;
  clearAfter: (id: unknown) => void;
  clearEvery: (id: unknown) => void;
};
const timers: Timers = {
  after: (fn, ms) => setTimeout(fn, ms),
  every: (fn, ms) => setInterval(fn, ms),
  clearAfter: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
  clearEvery: (id) => clearInterval(id as ReturnType<typeof setInterval>),
};
export function pollClientUpdates(
  check: () => Promise<ClientRelease | null>,
  receive: (release: ClientRelease | null) => void,
  clock = timers,
) {
  let stopped = false;
  let running = false;
  const run = async () => {
    if (stopped || running) return;
    running = true;
    try {
      const release = await check();
      if (!stopped) receive(release);
    } catch {
      // Native code records offline checks; do not interrupt the interface.
    } finally {
      running = false;
    }
  };
  const initial = clock.after(() => void run(), INITIAL_UPDATE_DELAY);
  const recurring = clock.every(() => void run(), UPDATE_CHECK_INTERVAL);
  return () => {
    stopped = true;
    clock.clearAfter(initial);
    clock.clearEvery(recurring);
  };
}
