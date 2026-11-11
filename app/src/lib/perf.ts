// A small frame-time meter, to check that animations stay smooth on WebKitGTK.
// Only used in dev builds (`pnpm tauri dev`): open the dev tools (right click →
// Inspect, or Ctrl+Shift+I) and look in the console after switching weeks.

/** Record the frames for `ms` milliseconds and log how smooth they were. */
export function measureFrames(label: string, ms = 400) {
  const times: number[] = [];
  const start = performance.now();

  function frame(time: number) {
    times.push(time);
    if (time - start < ms) requestAnimationFrame(frame);
    else report();
  }

  function report() {
    const gaps = times.slice(1).map((time, i) => time - times[i]);
    if (gaps.length === 0) return;
    const average = gaps.reduce((sum, gap) => sum + gap, 0) / gaps.length;
    const longest = Math.max(...gaps);
    // A frame that takes longer than ~1.5 frames at 60 fps is one you can see stutter.
    const dropped = gaps.filter((gap) => gap > 25).length;
    console.info(
      `[perf] ${label}: ${(1000 / average).toFixed(0)} fps average, ` +
        `longest frame ${longest.toFixed(1)} ms, ${dropped} slow frames of ${gaps.length}`,
    );
  }

  requestAnimationFrame(frame);
}
