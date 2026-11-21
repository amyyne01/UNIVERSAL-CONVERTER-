//! The dashboard speed sparkline's maths: Electron's `src/lib/sparkline.ts`, so each look draws the
//! same line from the same samples. Proven against recorded cases (`uilib_golden`).

/// Samples in the window; the newest sits at the right edge.
pub const SPARK_SAMPLES: usize = 120;

/// The smallest 1, 2, 5 times a power of ten at or above `v` (`niceCeil`); 1 for anything not above 0.
pub fn nice_ceil(v: f64) -> f64 {
    if !(v > 0.0) {
        return 1.0;
    }
    let pow = 10f64.powf(v.log10().floor());
    let f = v / pow;
    (if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    }) * pow
}

/// Eases the drawn ceiling toward its target so a rescale glides (`stepCeiling`, `k` is 0.3 in the app).
pub fn step_ceiling(cur: f64, target: f64, k: f64) -> f64 {
    if !(cur > 0.0) || (target - cur).abs() < target * 0.01 {
        return target;
    }
    cur + (target - cur) * k
}

/// `Number.prototype.toFixed(2)`: an exact tie rounds up (0.125 is 0.13, Rust gives 0.12), NaN and
/// the infinities are words, and a magnitude from 1e21 is written the way `String(x)` writes it.
fn to_fixed2(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.into();
    }
    if x == 0.0 {
        return "0.00".into();
    }
    if x.abs() >= 1e21 {
        return js_number(x);
    }
    let (sign, v) = if x < 0.0 { ("-", -x) } else { ("", x) };
    // Eighths with an odd numerator are the exact ties at two decimals.
    let tie = (v * 8.0).fract() == 0.0 && (v * 4.0).fract() != 0.0;
    format!("{sign}{:.2}", if tie { v + 0.001 } else { v })
}

/// `String(x)` for a magnitude of 1e21 or more: `1e+21`, `1.5e+25`.
fn js_number(x: f64) -> String {
    let s = format!("{x:e}");
    match s.split_once('e') {
        Some((m, e)) if !e.starts_with('-') => format!("{m}e+{e}"),
        _ => s,
    }
}

/// The polyline over the fixed window: `M118 0.00L119 31.50` (`sparkPath`).
pub fn spark_path(samples: &[f64], ceil: f64, height: f64) -> String {
    let off = SPARK_SAMPLES as i64 - samples.len() as i64;
    let min = |a: f64, b: f64| {
        if a.is_nan() || b.is_nan() {
            f64::NAN
        } else if a < b {
            a
        } else {
            b
        }
    };
    samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            format!("{}{} {}", if i == 0 { 'M' } else { 'L' }, off + i as i64, to_fixed2(height - (min(s, ceil) / ceil) * height))
        })
        .collect()
}
