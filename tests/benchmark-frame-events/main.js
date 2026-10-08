import { configure, on } from "deflorta";

configure({ id: "benchmark-frame-events", autosave: false });
let intervals = [];
on("frame", event => intervals.push(event.frameMs));
on("key", event => {
  if (!event.down) return;
  if (event.key === "enable") configure({ frameEvents: true });
  if (event.key === "disable") configure({ frameEvents: false });
  if (event.key === "state") configure({ title: JSON.stringify(intervals) });
  if (event.key === "clear") intervals = [];
});
