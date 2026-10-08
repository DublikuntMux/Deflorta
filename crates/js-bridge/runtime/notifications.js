import { native } from "deflorta/core";

const STATES = new Set(["info", "loading", "success", "error"]);

function notificationState(message, options) {
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError("Notification options must be an object");
  }
  if (typeof message !== "string") {
    throw new TypeError("Notification message must be a string");
  }
  const state = options.state ?? "info";
  if (!STATES.has(state)) {
    throw new TypeError(`Unknown notification state: ${state}`);
  }
  const duration = options.duration === undefined
    ? (state === "loading" ? null : 2)
    : options.duration;
  if (duration !== null &&
      (typeof duration !== "number" || !Number.isFinite(duration) || duration < 0)) {
    throw new TypeError("Notification duration must be a finite, non-negative number or null");
  }
  return { message, state, duration };
}

export function notify(message, options = {}) {
  let current = notificationState(message, options);
  const id = native.notifications.create(current.message, current);
  let dismissed = false;
  return Object.freeze({
    id,
    update(changes) {
      if (dismissed) return;
      if (changes === null || typeof changes !== "object" || Array.isArray(changes)) {
        throw new TypeError("Notification update must be an object");
      }
      const next = { ...current, ...changes };
      // A new state uses its normal lifetime unless a duration is supplied.
      if (changes.state !== undefined && changes.state !== current.state &&
          changes.duration === undefined) {
        delete next.duration;
      }
      const updated = notificationState(next.message, next);
      native.notifications.update(id, updated.message, updated);
      current = updated;
    },
    dismiss() {
      if (dismissed) return;
      native.notifications.dismiss(id);
      dismissed = true;
    },
  });
}
