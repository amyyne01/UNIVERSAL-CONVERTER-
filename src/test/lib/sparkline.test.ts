import { niceCeil, stepCeiling, sparkPath, SPARK_SAMPLES } from '@/lib/sparkline';

describe('sparkline math', () => {
  it('niceCeil rounds up to 1/2/5 x 10^n', () => {
    expect(niceCeil(0)).toBe(1);
    expect(niceCeil(1)).toBe(1);
    expect(niceCeil(1.1)).toBe(2);
    expect(niceCeil(3_400_000)).toBe(5_000_000);
    expect(niceCeil(5_000_001)).toBe(10_000_000);
    expect(niceCeil(120)).toBe(200);
  });

  it('stepCeiling glides toward the target and snaps when close', () => {
    const a = stepCeiling(10, 20);
    expect(a).toBeGreaterThan(10);
    expect(a).toBeLessThan(20);
    expect(stepCeiling(19.95, 20)).toBe(20);
    expect(stepCeiling(0, 20)).toBe(20);
  });

  it('sparkPath right-aligns samples and clamps to the ceiling', () => {
    const p = sparkPath([0, 50, 500], 100, 10);
    expect(p).toBe(`M${SPARK_SAMPLES - 3} 10.00L${SPARK_SAMPLES - 2} 5.00L${SPARK_SAMPLES - 1} 0.00`);
    expect(sparkPath([], 100, 10)).toBe('');
  });
});
