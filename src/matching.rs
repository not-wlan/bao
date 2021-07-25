use crate::error::BaoError;
use regex::bytes::Regex;
use serde_derive::{Deserialize, Serialize};
use std::convert::TryInto;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct BaoSymbol {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) pattern: String,
    #[serde(default)]
    pub(crate) start_rva:usize,
    #[serde(default)]
    extra: isize,
    #[serde(default)]
    pub offsets: Vec<isize>,
    #[serde(default)]
    pub relative: bool,
    #[serde(default)]
    pub rip_relative: bool,
    #[serde(default)]
    pub rip_offset: isize,
}

fn add_signed(first: usize, second: isize) -> Option<usize> {
    if second < 0 {
        first.checked_sub(second.wrapping_abs() as usize)
    } else {
        first.checked_add(second as usize)
    }
}

impl BaoSymbol {
    pub fn find(&self, data: &[u8], imagebase: usize) -> Result<(usize, bool), BaoError> {
        let mut va = false;
        if self.start_rva != 0 {
            return Ok((self.start_rva + imagebase, true));
        }
        let peek_bytes = |offset| -> Result<u32, BaoError> {
            let bfr: &[u8; 4] =
                &data[offset..offset + 4]
                    .try_into()
                    .map_err(|_| BaoError::BadPattern {
                        pattern: self.pattern.clone(),
                    })?;

            Ok(u32::from_le_bytes(*bfr))
        };

        let regex = self
            .pattern
            .split_ascii_whitespace()
            .map(|nibble| match &nibble {
                &"?" | &"??" => Ok(".".to_string()),
                nibble => {
                    if nibble.len() == 2 {
                        Ok(format!("\\x{}", nibble))
                    } else {
                        Err(BaoError::BadPattern {
                            pattern: self.pattern.clone(),
                        })
                    }
                }
            })
            .collect::<Result<String, BaoError>>()?;
        let regex = format!("(?s-u){}", regex);
        let regex = Regex::new(&regex).map_err(|_| BaoError::BadPattern {
            pattern: self.pattern.clone(),
        })?;

        if let Some(result) = regex.find(data) {
            let mut result = result.start();

            for deref in &self.offsets {
                result = add_signed(result, *deref).ok_or(BaoError::BadPattern {
                    pattern: self.pattern.clone(),
                })?;
                result = peek_bytes(result)? as usize;
                va = true;
            }

            result = add_signed(result, self.extra).ok_or(BaoError::BadPattern {
                pattern: self.pattern.clone(),
            })?;

            if self.rip_relative {
                result = (result as u32).wrapping_add(peek_bytes(result)?) as usize;
                result = add_signed(result, self.rip_offset).ok_or(BaoError::BadPattern {
                    pattern: self.pattern.clone(),
                })?;
            }

            if self.relative {
                result -= imagebase;
                va = true;
            }

            Ok((result, va))
        } else {
            Err(BaoError::PatternNotFound {
                name: self.name.clone(),
            })
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct BaoConfiguration {
    #[serde(default)]
    pub(crate) functions: Vec<BaoSymbol>,
    #[serde(default)]
    pub(crate) globals: Vec<BaoSymbol>,
}
