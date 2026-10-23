// The two framer-motion easings the app uses, named after their CSS twins in
// index.css (--ease-out-expo / --ease-out-quint). One home: never inline the tuples.
export type Ease = [number, number, number, number];
export const EASE_OUT_EXPO: Ease = [0.16, 1, 0.3, 1];
export const EASE_OUT_QUINT: Ease = [0.22, 1, 0.36, 1];
