export type LatencyPhase =
  | "idle"
  | "starting"
  | "running"
  | "cancelling"
  | "recovering";
export type LatencyState = { phase: LatencyPhase; id: string };
export type LatencyAction =
  | { type: "start" }
  | { type: "accepted"; id: string }
  | { type: "cancel" }
  | { type: "recover" }
  | { type: "finished" }
  | { type: "failed" };
export const initialLatency: LatencyState = { phase: "idle", id: "" };
export function latencyReducer(
  state: LatencyState,
  action: LatencyAction,
): LatencyState {
  switch (action.type) {
    case "start":
      return { phase: "starting", id: "" };
    case "accepted":
      return { phase: "running", id: action.id };
    case "cancel":
      return { ...state, phase: "cancelling" };
    case "recover":
      return { ...state, phase: "recovering" };
    case "finished":
    case "failed":
      return initialLatency;
  }
}
