use crate::{TagError, TagResult};

/// A modulo/residual validation rule for clean addresses or words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResidualClass {
    /// Modulus used for validation.
    pub modulus: usize,
    /// Expected residue.
    pub residue: usize,
}

impl ResidualClass {
    /// Create a residual class rule.
    pub const fn new(modulus: usize, residue: usize) -> TagResult<Self> {
        if modulus == 0 {
            return Err(TagError::ZeroModulus);
        }
        if residue >= modulus {
            return Err(TagError::BadResidue { modulus, residue });
        }
        Ok(Self { modulus, residue })
    }

    /// Check whether `word % modulus == residue`.
    pub const fn check(self, word: usize) -> bool {
        word % self.modulus == self.residue
    }

    /// Return the residue of `word` under this class's modulus.
    pub const fn residue_of(self, word: usize) -> usize {
        word % self.modulus
    }
}

/// Maps a residual class into a selector table.
#[derive(Debug, Clone)]
pub struct ResidualRouter {
    modulus: usize,
    selectors: Vec<u8>,
}

impl ResidualRouter {
    /// Create a router with `modulus` slots initialized to zero.
    pub fn new(modulus: usize) -> TagResult<Self> {
        if modulus == 0 {
            return Err(TagError::ZeroModulus);
        }
        Ok(Self {
            modulus,
            selectors: vec![0; modulus],
        })
    }

    /// Assign a selector for one residue.
    pub fn set(mut self, residue: usize, selector: u8) -> TagResult<Self> {
        if residue >= self.modulus {
            return Err(TagError::BadResidue {
                modulus: self.modulus,
                residue,
            });
        }
        self.selectors[residue] = selector;
        Ok(self)
    }

    /// Select based on `word % modulus`.
    pub fn selector_for(&self, word: usize) -> u8 {
        self.selectors[word % self.modulus]
    }

    /// Return the configured modulus.
    pub const fn modulus(&self) -> usize {
        self.modulus
    }
}
