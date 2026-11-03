pub fn sm2_update(
    interval: i64,
    ease: f64,
    lapses: i64,
    correct: bool,
) -> (i64, f64, i64, &'static str) {
    if correct {
        let new_interval = if interval <= 1 {
            1
        } else {
            (interval as f64 * ease).round() as i64
        };
        let new_interval = new_interval.clamp(1, 365);
        let new_ease = (ease + 0.1).clamp(1.3, 2.8);
        (new_interval, new_ease, lapses, "review")
    } else {
        (1, (ease - 0.2).clamp(1.3, 2.8), lapses + 1, "relearn")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn sm2_correct_increases() {
        let (iv, ease, _, st) = sm2_update(2, 2.5, 0, true);
        assert!(iv >= 2);
        assert!(ease >= 2.5);
        assert_eq!(st, "review");
    }

    #[test]
    fn sm2_wrong_resets() {
        let (iv, _, lapses, st) = sm2_update(5, 2.5, 0, false);
        assert_eq!(iv, 1);
        assert_eq!(lapses, 1);
        assert_eq!(st, "relearn");
    }

    #[test]
    fn sm2_correct_caps_interval() {
        let (iv, _, _, _) = sm2_update(365, 2.8, 0, true);
        assert!(iv <= 365);
    }

    #[test]
    fn sm2_wrong_ease_floor() {
        let (_, ease, _, _) = sm2_update(5, 1.3, 0, false);
        assert!(ease >= 1.3);
    }

    #[test]
    fn sm2_wrong_increments_lapses() {
        let (_, _, lapses, _) = sm2_update(5, 2.5, 3, false);
        assert_eq!(lapses, 4);
    }

    #[test]
    fn sm2_correct_preserves_lapses() {
        let (_, _, lapses, _) = sm2_update(5, 2.5, 7, true);
        assert_eq!(lapses, 7);
    }

    proptest! {
        #[test]
        fn sm2_bounds(interval in 0i64..1000, ease in 1.0f64..3.0, correct in proptest::bool::ANY) {
            let (new_interval, new_ease, _, _) = sm2_update(interval, ease, 0, correct);
            prop_assert!((1..=365).contains(&new_interval));
            prop_assert!((1.0..=2.8).contains(&new_ease));
        }
    }
}
