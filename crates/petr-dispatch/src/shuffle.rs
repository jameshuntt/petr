use crate::{DispatchResult, OpFrame, OperationTable};

/// A boot/session-local permutation over 256 bytecode selectors.
#[derive(Debug, Clone)]
pub struct SelectorPermutation {
    logical_to_physical: [u8; 256],
    physical_to_logical: [u8; 256],
}

impl SelectorPermutation {
    /// Create a deterministic shuffled permutation from a seed.
    pub fn from_seed(seed: u64) -> Self {
        let mut arr = [0u8; 256];
        let mut i = 0usize;
        while i < 256 {
            arr[i] = i as u8;
            i += 1;
        }
        let mut x = seed;
        let mut j = 255usize;
        while j > 0 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let k = (x as usize) % (j + 1);
            arr.swap(j, k);
            j -= 1;
        }
        let mut inv = [0u8; 256];
        for (logical, physical) in arr.iter().enumerate() {
            inv[*physical as usize] = logical as u8;
        }
        Self {
            logical_to_physical: arr,
            physical_to_logical: inv,
        }
    }

    /// Encode a logical selector into the shuffled physical selector.
    pub fn encode(&self, logical: u8) -> u8 {
        self.logical_to_physical[logical as usize]
    }

    /// Decode a physical selector back to the logical selector.
    pub fn decode(&self, physical: u8) -> u8 {
        self.physical_to_logical[physical as usize]
    }
}

/// Operation table whose externally visible bytecode selectors are shuffled.
#[derive(Debug, Clone)]
pub struct ShuffledOperationTable {
    table: OperationTable,
    permutation: SelectorPermutation,
}

impl ShuffledOperationTable {
    /// Create a shuffled table from an existing logical table.
    pub fn new(logical: OperationTable, seed: u64) -> Self {
        Self {
            table: logical,
            permutation: SelectorPermutation::from_seed(seed),
        }
    }

    /// Encode logical selector for storage/transmission.
    pub fn encode_selector(&self, logical: u8) -> u8 {
        self.permutation.encode(logical)
    }

    /// Execute using a shuffled physical selector.
    pub fn execute_physical(&self, physical: u8, frame: OpFrame) -> DispatchResult<OpFrame> {
        let logical = self.permutation.decode(physical);
        self.table.execute(logical, frame)
    }
}
