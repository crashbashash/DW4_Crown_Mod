//! Narrow Lua 4 chunk editor. Original constants and instructions come from
//! the selected disc; inserted instructions are authored here, not game blobs.
use super::super::{bad, need, u32at, Result};

pub const BIAS: i64 = (1 << 25) - 1;
pub fn unsigned(op: u32, arg: usize) -> u32 {
    op | ((arg as u32) << 6)
}
pub fn signed(op: u32, arg: i64) -> u32 {
    op | (((BIAS + arg) as u32) << 6)
}
pub fn call(stack: usize, results: usize) -> u32 {
    2 | ((stack as u32) << 15) | ((results as u32) << 6)
}
fn jump(op: u32) -> bool {
    (32..=42).contains(&op) || (44..=47).contains(&op)
}

pub struct Chunk {
    prefix: Vec<u8>,
    middle: Vec<u8>,
    stack_offset: usize,
    pub strings: Vec<Vec<u8>>,
    pub code: Vec<u32>,
}
impl Chunk {
    pub fn read(b: &[u8]) -> Result<Self> {
        need(
            b.get(..13) == Some(b"\x1bLua@\x01\x04\x04\x04\x20\x06\x09\x08"),
            "unexpected river Lua header",
        )?;
        let mut q = 21;
        fn string(b: &[u8], q: &mut usize) -> Result<Vec<u8>> {
            let n = u32at(b, *q)? as usize;
            *q += 4;
            let value = b
                .get(*q..*q + n)
                .ok_or_else(|| bad("truncated Lua string"))?
                .to_vec();
            *q += n;
            Ok(value)
        }
        string(b, &mut q)?;
        q += 4;
        need(
            u32at(b, q)? == 0 && b.get(q + 4) == Some(&0),
            "river Lua has parameters",
        )?;
        q += 5;
        let stack_offset = q;
        q += 4;
        need(
            u32at(b, q)? == 0 && u32at(b, q + 4)? == 0,
            "river Lua has debug records",
        )?;
        q += 8;
        let prefix = b[..q].to_vec();
        let count = u32at(b, q)? as usize;
        q += 4;
        need(count < 4096, "too many river Lua strings")?;
        let strings = (0..count)
            .map(|_| string(b, &mut q))
            .collect::<Result<Vec<_>>>()?;
        let start = q;
        let numbers = u32at(b, q)? as usize;
        q += 4;
        need(numbers < 4096, "too many river Lua numbers")?;
        q += 8 * numbers;
        need(u32at(b, q)? == 0, "river Lua has nested prototypes")?;
        q += 4;
        let middle = b[start..q].to_vec();
        let count = u32at(b, q)? as usize;
        q += 4;
        need(
            count < 100_000 && q + 4 * count == b.len(),
            "unexpected river Lua extent",
        )?;
        let code = (0..count)
            .map(|i| u32at(b, q + 4 * i))
            .collect::<Result<Vec<_>>>()?;
        need(code.last() == Some(&0), "river Lua does not end")?;
        for (pc, &ins) in code.iter().enumerate() {
            if jump(ins & 63) {
                let target = pc as i64 + 1 + ((ins >> 6) as i64 - BIAS);
                need(
                    target >= 0 && target < code.len() as i64,
                    "river Lua branch outside code",
                )?;
            }
        }
        Ok(Self {
            prefix,
            middle,
            stack_offset,
            strings,
            code,
        })
    }
    pub fn text(&self, i: usize) -> Result<&str> {
        let b = self
            .strings
            .get(i)
            .ok_or_else(|| bad("river Lua string index"))?;
        std::str::from_utf8(b.strip_suffix(&[0]).unwrap_or(b))
            .map_err(|_| bad("non-ASCII river Lua constant"))
    }
    pub fn string(&mut self, s: &str) -> usize {
        if let Some(i) = self
            .strings
            .iter()
            .position(|b| b.strip_suffix(&[0]).unwrap_or(b) == s.as_bytes())
        {
            return i;
        }
        let mut b = s.as_bytes().to_vec();
        b.push(0);
        self.strings.push(b);
        self.strings.len() - 1
    }
    pub fn get(&mut self, s: &str) -> u32 {
        unsigned(12, self.string(s))
    }
    pub fn set(&mut self, s: &str) -> u32 {
        unsigned(19, self.string(s))
    }
    pub fn literal(&mut self, s: &str) -> u32 {
        unsigned(7, self.string(s))
    }
    pub fn method(&mut self, s: &str) -> [u32; 2] {
        [self.get("be"), unsigned(16, self.string(s))]
    }
    pub fn is(&self, ins: u32, op: u32, s: &str) -> bool {
        ins & 63 == op && self.text((ins >> 6) as usize).ok() == Some(s)
    }
    pub fn write(&self, code: &[u32]) -> Result<Vec<u8>> {
        let mut b = self.prefix.clone();
        let stack = u32at(&b, self.stack_offset)?.max(40);
        b[self.stack_offset..self.stack_offset + 4].copy_from_slice(&stack.to_le_bytes());
        b.extend((self.strings.len() as u32).to_le_bytes());
        for s in &self.strings {
            b.extend((s.len() as u32).to_le_bytes());
            b.extend(s);
        }
        b.extend(&self.middle);
        b.extend((code.len() as u32).to_le_bytes());
        for v in code {
            b.extend(v.to_le_bytes());
        }
        Self::read(&b)?;
        Ok(b)
    }
    /// Before-insertions run for incoming branches. After-insertions run only
    /// on fallthrough, so a jump past a death call never closes a live crown.
    /// Insertions preserve existing local stack slots. Every original branch
    /// is relocated, including exits and backward loop edges.
    pub fn rewrite(
        &self,
        before: &std::collections::BTreeMap<usize, Vec<u32>>,
        after: &std::collections::BTreeMap<usize, Vec<u32>>,
        replace: &std::collections::BTreeMap<usize, u32>,
    ) -> Result<Vec<u32>> {
        let mut positions = Vec::new();
        let mut out = Vec::new();
        for (pc, &ins) in self.code.iter().enumerate() {
            positions.push(out.len());
            if let Some(v) = before.get(&pc) {
                out.extend(v);
            }
            out.push(*replace.get(&pc).unwrap_or(&ins));
            if let Some(v) = after.get(&pc) {
                out.extend(v);
            }
        }
        for (pc, &old) in self.code.iter().enumerate() {
            let at = positions[pc] + before.get(&pc).map_or(0, Vec::len);
            if jump(old & 63) {
                let target = (pc as i64 + 1 + (old >> 6) as i64 - BIAS) as usize;
                out[at] = signed(out[at] & 63, positions[target] as i64 - at as i64 - 1);
            }
        }
        Ok(out)
    }
}

/// An inserted conditional uses a stock comparison jump to skip its body.
pub fn conditional(code: &mut Vec<u32>, op: u32, body: impl FnOnce(&mut Vec<u32>)) {
    let at = code.len();
    code.push(0);
    body(code);
    code[at] = signed(op, code.len() as i64 - at as i64 - 1);
}
