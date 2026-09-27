// Seeded RNG (xorshift64*) - deterministic world generation.
pub struct Rng {
    s: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self { s: seed.max(1) }
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.s;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.s = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    /// Uniform [0, 1)
    #[inline]
    pub fn f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u64 << 24) as f32)
    }

    /// Uniform [a, b)
    #[inline]
    pub fn f32b(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f32()
    }

    /// Uniform integer [a, b] inclusive
    #[inline]
    pub fn u64b(&mut self, a: u64, b: u64) -> u64 {
        if b <= a {
            return a;
        }
        a + self.next_u64() % (b - a + 1)
    }

    #[inline]
    pub fn bool_p(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}

/// Hash a string into a u64 seed (for typed seeds).
pub fn hash_seed(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h.max(1)
}
