import { useEffect, useState } from "react";
export function useClock(live: boolean, started: number, elapsed = 0) {
  const [now, setNow] = useState(0);
  useEffect(() => { if (!live) return; setNow(Date.now()); const timer = setInterval(() => setNow(Date.now()), 1000); return () => clearInterval(timer); }, [live]);
  const seconds = live && now ? Math.max(0, Math.floor(now / 1000 - started)) : elapsed;
  return `${String(Math.floor(seconds / 60)).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}`;
}
