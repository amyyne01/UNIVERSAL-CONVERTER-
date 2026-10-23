// Pure maths for the dashboard speed sparkline (rendering lives in SpeedSparkline).

export const SPARK_SAMPLES = 120;

/** Smallest 1/2/5 x 10^n at or above v - a "nice" axis ceiling. */
export function niceCeil(v: number): number {
  if (!(v > 0)) return 1;
  const pow = 10 ** Math.floor(Math.log10(v));
  const f = v / pow;
  return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * pow;
}

/** Ease the drawn ceiling toward its target so a rescale glides instead of jumping. */
export function stepCeiling(cur: number, target: number, k = 0.3): number {
  if (!(cur > 0) || Math.abs(target - cur) < target * 0.01) return target;
  return cur + (target - cur) * k;
}

/** SVG polyline path over a fixed SPARK_SAMPLES-wide window, newest sample at the right edge. */
export function sparkPath(samples: number[], ceil: number, height: number): string {
  const off = SPARK_SAMPLES - samples.length;
  return samples
    .map((s, i) => `${i ? 'L' : 'M'}${off + i} ${(height - (Math.min(s, ceil) / ceil) * height).toFixed(2)}`)
    .join('');
}
