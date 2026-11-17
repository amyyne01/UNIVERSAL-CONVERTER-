//! The 165 numbered rows of `MOTION.md`, with their tier (the command of its 5.4 gives the ids;
//! the tier is the "Priority" column). `docs/` is gitignored, so the table lives here.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Core,
    Polish,
    Delight,
    /// ALL-CTL-11: a deliberate "nothing" (a save that always succeeds needs no confirmation).
    Nothing,
}

pub const ROWS: &[(&str, Tier)] = &[
    ("ALL-BTN-01", Tier::Core),
    ("ALL-BTN-02", Tier::Core),
    ("ALL-BTN-03", Tier::Core),
    ("ALL-BTN-04", Tier::Polish),
    ("ALL-BTN-05", Tier::Polish),
    ("ALL-BTN-06", Tier::Polish),
    ("ALL-BTN-07", Tier::Delight),
    ("ALL-FOC-01", Tier::Core),
    ("ALL-FOC-02", Tier::Core),
    ("ALL-TIP-01", Tier::Core),
    ("ALL-FLD-01", Tier::Core),
    ("ALL-FLD-02", Tier::Core),
    ("ALL-FLD-03", Tier::Core),
    ("ALL-FLD-04", Tier::Polish),
    ("ALL-FLD-05", Tier::Polish),
    ("ALL-NAV-01", Tier::Core),
    ("ALL-NAV-02", Tier::Polish),
    ("ALL-NAV-03", Tier::Core),
    ("ALL-NAV-04", Tier::Core),
    ("ALL-NAV-05", Tier::Core),
    ("ALL-NAV-06", Tier::Polish),
    ("ALL-NAV-07", Tier::Core),
    ("ALL-ROW-01", Tier::Core),
    ("ALL-ROW-02", Tier::Core),
    ("ALL-ROW-03", Tier::Core),
    ("ALL-ROW-04", Tier::Core),
    ("ALL-ROW-05", Tier::Core),
    ("ALL-ROW-06", Tier::Polish),
    ("ALL-ROW-07", Tier::Polish),
    ("ALL-ROW-08", Tier::Polish),
    ("ALL-ROW-09", Tier::Core),
    ("ALL-ROW-10", Tier::Core),
    ("ALL-ROW-11", Tier::Polish),
    ("ALL-ROW-12", Tier::Core),
    ("ALL-ROW-13", Tier::Core),
    ("ALL-ROW-14", Tier::Core),
    ("ALL-ROW-15", Tier::Polish),
    ("ALL-ROW-16", Tier::Core),
    ("ALL-ROW-17", Tier::Polish),
    ("ALL-ROW-18", Tier::Polish),
    ("ALL-NUM-01", Tier::Core),
    ("ALL-NUM-02", Tier::Polish),
    ("ALL-NUM-03", Tier::Polish),
    ("ALL-NUM-04", Tier::Polish),
    ("ALL-NUM-05", Tier::Polish),
    ("ALL-NUM-06", Tier::Core),
    ("ALL-NUM-07", Tier::Delight),
    ("ALL-FND-01", Tier::Core),
    ("ALL-FND-02", Tier::Core),
    ("ALL-FND-03", Tier::Core),
    ("ALL-FND-04", Tier::Core),
    ("ALL-FND-05", Tier::Core),
    ("ALL-FND-06", Tier::Core),
    ("ALL-FND-07", Tier::Core),
    ("ALL-FND-08", Tier::Polish),
    ("ALL-FND-09", Tier::Polish),
    ("ALL-FND-10", Tier::Core),
    ("ALL-FND-11", Tier::Core),
    ("ALL-FND-12", Tier::Core),
    ("ALL-FND-13", Tier::Polish),
    ("ALL-FND-14", Tier::Delight),
    ("ALL-FND-15", Tier::Core),
    ("ALL-FND-16", Tier::Core),
    ("ALL-FND-17", Tier::Polish),
    ("ALL-CTL-01", Tier::Core),
    ("ALL-CTL-02", Tier::Core),
    ("ALL-CTL-03", Tier::Core),
    ("ALL-CTL-04", Tier::Core),
    ("ALL-CTL-05", Tier::Core),
    ("ALL-CTL-06", Tier::Core),
    ("ALL-CTL-07", Tier::Polish),
    ("ALL-CTL-08", Tier::Polish),
    ("ALL-CTL-09", Tier::Polish),
    ("ALL-CTL-10", Tier::Core),
    ("ALL-CTL-11", Tier::Nothing),
    ("ALL-CTL-12", Tier::Polish),
    ("ALL-OVL-01", Tier::Core),
    ("ALL-OVL-02", Tier::Core),
    ("ALL-OVL-03", Tier::Core),
    ("ALL-OVL-04", Tier::Core),
    ("ALL-OVL-05", Tier::Polish),
    ("ALL-OVL-06", Tier::Core),
    ("ALL-OVL-07", Tier::Polish),
    ("ALL-OVL-08", Tier::Polish),
    ("ALL-OVL-09", Tier::Core),
    ("ALL-OVL-10", Tier::Core),
    ("ALL-SWT-01", Tier::Core),
    ("ALL-SWT-02", Tier::Core),
    ("ALL-SWT-03", Tier::Core),
    ("ALL-SCR-01", Tier::Polish),
    ("ALL-SCR-02", Tier::Core),
    ("ALL-SCR-03", Tier::Core),
    ("ALL-WIN-01", Tier::Core),
    ("ALL-WIN-02", Tier::Core),
    ("ALL-WIN-03", Tier::Core),
    ("ALL-WIN-04", Tier::Polish),
    ("ALL-STA-01", Tier::Core),
    ("ALL-STA-02", Tier::Core),
    ("ALL-STA-03", Tier::Core),
    ("ALL-STA-04", Tier::Polish),
    ("ALL-STA-05", Tier::Polish),
    ("SIG-RNG-01", Tier::Core),
    ("SIG-RNG-02", Tier::Polish),
    ("SIG-NUM-01", Tier::Core),
    ("SIG-NUM-02", Tier::Polish),
    ("SIG-FLD-01", Tier::Polish),
    ("SIG-FLD-02", Tier::Polish),
    ("SIG-FND-01", Tier::Core),
    ("SIG-FND-02", Tier::Polish),
    ("SIG-ROW-01", Tier::Polish),
    ("SIG-ROW-02", Tier::Core),
    ("SIG-CTL-01", Tier::Polish),
    ("SIG-STA-01", Tier::Polish),
    ("SIG-HOV-01", Tier::Polish),
    ("SHF-RNG-01", Tier::Core),
    ("SHF-RNG-02", Tier::Delight),
    ("SHF-RNG-03", Tier::Core),
    ("SHF-STA-01", Tier::Core),
    ("SHF-ROW-01", Tier::Polish),
    ("SHF-HOV-01", Tier::Polish),
    ("SHF-HOV-02", Tier::Delight),
    ("SHF-WIN-01", Tier::Polish),
    ("SHF-CTL-01", Tier::Delight),
    ("ORB-RNG-01", Tier::Core),
    ("ORB-RNG-02", Tier::Core),
    ("ORB-FND-01", Tier::Core),
    ("ORB-FND-02", Tier::Polish),
    ("ORB-STA-01", Tier::Delight),
    ("ORB-HOV-01", Tier::Polish),
    ("ORB-NAV-01", Tier::Polish),
    ("BAR-NAV-01", Tier::Core),
    ("BAR-NAV-02", Tier::Core),
    ("BAR-NAV-03", Tier::Polish),
    ("BAR-OVL-01", Tier::Core),
    ("BAR-OVL-02", Tier::Core),
    ("BAR-OVL-03", Tier::Polish),
    ("BAR-FND-01", Tier::Core),
    ("BAR-FND-02", Tier::Core),
    ("BAR-NUM-01", Tier::Polish),
    ("BAR-ROW-01", Tier::Core),
    ("BAR-CTL-01", Tier::Polish),
    ("BAR-FLD-01", Tier::Polish),
    ("COL-ROW-01", Tier::Core),
    ("COL-ROW-02", Tier::Core),
    ("COL-ROW-03", Tier::Core),
    ("COL-ROW-04", Tier::Core),
    ("COL-ROW-05", Tier::Core),
    ("COL-STA-01", Tier::Polish),
    ("COL-NUM-01", Tier::Polish),
    ("COL-NUM-02", Tier::Core),
    ("COL-FND-01", Tier::Core),
    ("COL-ROW-06", Tier::Delight),
    ("STG-NAV-01", Tier::Core),
    ("STG-NAV-02", Tier::Core),
    ("STG-STG-01", Tier::Core),
    ("STG-STG-02", Tier::Core),
    ("STG-STA-01", Tier::Core),
    ("STG-SAT-01", Tier::Polish),
    ("STG-SAT-02", Tier::Polish),
    ("STG-ROW-01", Tier::Polish),
    ("STG-FND-01", Tier::Core),
    ("STG-FND-02", Tier::Core),
    ("STG-BTN-01", Tier::Core),
    ("STG-STG-03", Tier::Polish),
    ("STG-STG-04", Tier::Delight),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn count(t: Tier) -> usize {
        ROWS.iter().filter(|r| r.1 == t).count()
    }

    #[test]
    fn the_catalogue_has_165_unique_rows() {
        assert_eq!(ROWS.len(), 165);
        let ids: HashSet<_> = ROWS.iter().map(|r| r.0).collect();
        assert_eq!(ids.len(), ROWS.len(), "an id is listed twice");
    }

    /// MOTION.md 4.4: core 96, polish 59, delight 9, and the one "nothing". The "Foundation" block
    /// of MOTION-SYSTEM.md section 8 is build steps (M1 to M8), not rows: it has no ids.
    #[test]
    fn tiers_add_up_as_the_doc_says() {
        assert_eq!((count(Tier::Core), count(Tier::Polish), count(Tier::Delight), count(Tier::Nothing)), (96, 59, 9, 1));
        assert_eq!(ROWS.iter().find(|r| r.1 == Tier::Nothing).map(|r| r.0), Some("ALL-CTL-11"));
    }

    /// MOTION.md 4.4: shared 101, then each look's own rows.
    #[test]
    fn rows_per_group_match_the_doc() {
        let n = |p: &str| ROWS.iter().filter(|r| r.0.starts_with(p)).count();
        let got = [n("ALL-"), n("SIG-"), n("SHF-"), n("ORB-"), n("BAR-"), n("COL-"), n("STG-")];
        assert_eq!(got, [101, 13, 9, 7, 12, 10, 13]);
    }
}
