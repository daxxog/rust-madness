//! The port's `java.util.Random` and the `Math.random()` slot it seeds.
//!
//! `java.util.Random` is the Javadoc-specified 48-bit LCG, stable across
//! JDKs forever; `TJavaRandom.java` pins the literal vectors the mirror
//! replays. In the original, `Math.random()` delegates to ONE global
//! holder generator (JDK 8: `Math$RandomNumberGeneratorHolder
//! .randomNumberGenerator`) and every randomness consumer reads it —
//! `Medium.random()` (Medium.java:54-55), GameSparker (GameSparker.java:616)
//! and xtGraphics (xtGraphics.java:2289-2376), with no `new Random`
//! anywhere in reference/vineflower/.
//!
//! Port shape (documented divergence, notes/PHASE-B.md ledger):
//! - direct `Math.random()` consumers read the global slot
//!   ([`math_random`]), whose default is a fixed-seed xorshift32 for
//!   reproducible play instead of the stock unseeded `Random`;
//! - `Medium.random()`'s default also reads that slot (C3b), matching
//!   Java's single-stream interleaving; `Medium.rng` is the test seam for
//!   a private stream (the per-instance stand-in for TestRng.install and
//!   for the VecMedium/TenthsMedium subclass overrides).
//!
//! The slot is production state: `xtGraphics.random()` (the sc/proba
//! machine, xtGraphics.java:2289-2376) and the GameSparker finale stage
//! pick (GameSparker.java:616) read it. The TestRng mirror seam
//! (`install`/`uninstall`/`current_is_default`) stays `#[cfg(test)]`:
//! production never swaps the slot.

/// `java.util.Random` — the Javadoc-specified 48-bit LCG.
pub struct JavaRandom {
    seed: i64,
}

/// `0x1.0p-53` — nextDouble's scale factor (exponent 970 = 2^-53).
const TWO_POW_NEG_53: f64 = f64::from_bits(0x3CA0_0000_0000_0000);

impl JavaRandom {
    const MULT: i64 = 0x5DEECE66D;
    const ADD: i64 = 0xB;
    const MASK: i64 = (1i64 << 48) - 1;

    pub fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: (seed ^ Self::MULT) & Self::MASK,
        }
    }

    /// `setSeed(long)` — restarts the stream (TJavaRandom's reseedRepeat).
    pub fn set_seed(&mut self, seed: i64) {
        self.seed = (seed ^ Self::MULT) & Self::MASK;
    }

    /// `next(bits)`: advance, then `(int)(seed >>> (48 - bits))`. The
    /// masked seed is always positive, so the arithmetic shift is the
    /// unsigned one Java means.
    fn next_bits(&mut self, bits: u32) -> i32 {
        self.seed = self.seed.wrapping_mul(Self::MULT).wrapping_add(Self::ADD) & Self::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `nextDouble()` = `((next(26) << 27) + next(27)) * 0x1.0p-53`.
    pub fn next_double(&mut self) -> f64 {
        let hi = self.next_bits(26) as i64;
        let lo = self.next_bits(27) as i64;
        ((hi << 27).wrapping_add(lo) as f64) * TWO_POW_NEG_53
    }

    /// `nextInt(bound)`: the power-of-two arm, then the rejection loop
    /// whose rejection test is INT OVERFLOW (`u - r + m < 0` wraps
    /// negative for over-represented candidates — the JDK's exact code).
    pub fn next_int(&mut self, bound: i32) -> i32 {
        let mut r = self.next_bits(31);
        let m = bound.wrapping_sub(1);
        if (bound & m) == 0 {
            r = ((bound as i64 * r as i64) >> 31) as i32;
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) < 0 {
                    u = self.next_bits(31);
                } else {
                    break;
                }
            }
        }
        r
    }

    /// `nextBoolean()` = `next(1) != 0`.
    pub fn next_boolean(&mut self) -> bool {
        self.next_bits(1) != 0
    }

    /// `nextLong()` = `((long)next(32) << 32) + next(32)` — the first
    /// word sign-extends.
    pub fn next_long(&mut self) -> i64 {
        ((self.next_bits(32) as i64) << 32).wrapping_add(self.next_bits(32) as i64)
    }
}

/// A `Medium`'s entropy source for `random()` — the per-medium stand-in
/// for Java's single global `Math.random()` (see the module header). The
/// default (None) is the per-medium xorshift32 in `Medium.seed`.
pub enum RngSource {
    /// TestRng.install(seed) bridge: the walk draws from this stream.
    Java(JavaRandom),
    /// VecMedium/TenthsMedium: random() is REPLACED by this wrapping
    /// vector (the walk and cntrn are bypassed entirely).
    Vector(Vec<f32>, usize),
}

// ── the Math.random() slot ─────────────────────────────────────────────

/// Where the slot's `Math.random()` draws from.
enum SlotSource {
    /// The port's default generator (documented divergence: the stock
    /// holder uses an unseeded `Random`).
    Xorshift(u32),
    /// `TestRng.install(seed)`: the holder points at `Random(seed)`. Test
    /// seam only — production never swaps the slot.
    #[cfg(test)]
    Java(JavaRandom),
}

struct SlotState {
    current: SlotSource,
    /// The first-saved source `install` restores on uninstall — the
    /// TestRng save-once contract (TestRng.java:47-53). Test seam only.
    #[cfg(test)]
    saved: Option<SlotSource>,
}

thread_local! {
    static SLOT: std::cell::RefCell<SlotState> = const { std::cell::RefCell::new(SlotState {
        current: SlotSource::Xorshift(0x5eed_1234),
        #[cfg(test)]
        saved: None,
    }) };
}

/// `Math.random()` — the original's single randomness source, the
/// `Math$RandomNumberGeneratorHolder.randomNumberGenerator` holder every
/// direct consumer reads.
pub fn math_random() -> f64 {
    SLOT.with_borrow_mut(|s| match &mut s.current {
        SlotSource::Xorshift(seed) => {
            let mut x = *seed;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            *seed = x;
            (x & 0xFFFF) as f64 / 65536.0
        }
        #[cfg(test)]
        SlotSource::Java(r) => r.next_double(),
    })
}

/// The `TestRng` mirror seam: `install` swaps the slot in test builds only.
#[cfg(test)]
pub(crate) mod slot {
    use super::{JavaRandom, SLOT, SlotSource};

    /// `TestRng.install(seed)` with save-once semantics: the FIRST-saved
    /// source is what every uninstall restores (TestRng.java:47-53).
    /// Dropping the guard is uninstall(); scopes nest like the Java
    /// try/finally pattern.
    pub struct RngGuard;

    pub fn install(seed: i64) -> RngGuard {
        SLOT.with_borrow_mut(|s| {
            let next = SlotSource::Java(JavaRandom::new(seed));
            if s.saved.is_none() {
                s.saved = Some(std::mem::replace(&mut s.current, next));
            } else {
                s.current = next;
            }
        });
        RngGuard
    }

    impl Drop for RngGuard {
        fn drop(&mut self) {
            SLOT.with_borrow_mut(|s| {
                if let Some(saved) = s.saved.take() {
                    s.current = saved;
                }
            });
        }
    }

    /// True when the slot holds the port's default generator — the
    /// uninstall-identity pins' observable.
    pub fn current_is_default() -> bool {
        SLOT.with_borrow(|s| matches!(s.current, SlotSource::Xorshift(_)))
    }
}

#[cfg(test)]
pub(crate) use slot::{current_is_default, install};

#[cfg(test)]
mod probes {
    use super::*;

    /// Random(42) Javadoc vectors (TJavaRandom.lcgVectors) — doubles
    /// bit-exact, nextInt both arms, nextBoolean, nextLong.
    #[test]
    fn lcg_javadoc_vectors() {
        let mut d = JavaRandom::new(42);
        assert_eq!(
            d.next_double().to_bits(),
            0x3fe74833a06ff457,
            "next_double[0]"
        );
        assert_eq!(
            d.next_double().to_bits(),
            0x3fe5dcf778622e01,
            "next_double[1]"
        );
        assert_eq!(JavaRandom::new(42).next_int(100), 30, "nextInt(100)[0]");
        assert_eq!(JavaRandom::new(42).next_int(16), 11, "nextInt(16) pow2");
        assert_eq!(
            JavaRandom::new(42).next_int(1_000_000_000),
            562431130,
            "nextInt(1e9) rejection"
        );
        assert!(JavaRandom::new(42).next_boolean(), "nextBoolean[0]");
        assert_eq!(
            JavaRandom::new(42).next_long(),
            -5025562857975149833,
            "nextLong[0]"
        );
    }
}
