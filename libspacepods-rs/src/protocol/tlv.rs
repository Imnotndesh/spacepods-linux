/// TLV (Tag-Length-Value) parser for handshake response payloads.
///
/// Wire format per entry:
///   [tag: 1][len: 1][value: len bytes...]
#[derive(Debug, Clone)]
pub struct TlvParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> TlvParser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Get the next TLV entry.
    pub fn next(&mut self) -> Option<(u8, &'a [u8])> {
        if self.pos + 2 > self.data.len() {
            return None;
        }

        let tag = self.data[self.pos];
        let len = self.data[self.pos + 1] as usize;
        let val_start = self.pos + 2;

        if val_start + len > self.data.len() {
            return None;
        }

        let value = &self.data[val_start..val_start + len];
        self.pos = val_start + len;

        Some((tag, value))
    }

    /// Read a single-byte integer value for the given tag.
    pub fn get_int(&mut self, target_tag: u8) -> Option<u8> {
        while let Some((tag, value)) = self.next() {
            if tag == target_tag && !value.is_empty() {
                return Some(value[0]);
            }
        }
        None
    }

    /// Read a raw byte slice for the given tag.
    pub fn get_bytes(&mut self, target_tag: u8) -> Option<&'a [u8]> {
        while let Some((tag, value)) = self.next() {
            if tag == target_tag {
                return Some(value);
            }
        }
        None
    }

    /// Collect all remaining entries into a Vec.
    pub fn collect_all(&mut self) -> Vec<(u8, &'a [u8])> {
        let mut entries = Vec::new();
        while let Some(entry) = self.next() {
            entries.push(entry);
        }
        entries
    }

    /// Check if we've consumed all data.
    pub fn is_exhausted(&self) -> bool {
        self.pos >= self.data.len()
    }
}
