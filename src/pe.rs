use crate::{error::BaoError, matching::BaoSymbol};
use goblin::pe::PE;
use std::ops::Deref;

pub struct BaoPE<'a>(PE<'a>);

impl<'a> From<PE<'a>> for BaoPE<'a> {
    fn from(pe: PE<'a>) -> Self {
        BaoPE(pe)
    }
}

pub(crate) struct SearchResult {
    pub(crate) offset: u32,
    pub(crate) name: String,
    pub(crate) index: u16,
}

impl<'a> BaoPE<'a> {
    pub fn get_section_data(&self, offset: usize, va: bool) -> Option<(u32, u16)> {
        self.0
            .sections
            .iter()
            .enumerate()
            .find(|&(_, section)| {
                if va {
                    offset >= section.virtual_address as usize
                        && offset < (section.virtual_address + section.virtual_size) as usize
                } else {
                    offset >= section.pointer_to_raw_data as usize
                        && offset
                            < (section.pointer_to_raw_data + section.size_of_raw_data) as usize
                }
            })
            .map(|(i, section)| {
                (
                    (offset
                        - (if va {
                            section.virtual_address
                        } else {
                            section.pointer_to_raw_data
                        }) as usize) as u32,
                    (i + 1) as u16,
                )
            })
    }

    pub(crate) fn find_symbols(
        &self,
        symbols: Vec<BaoSymbol>,
        data: &[u8],
        warnings: &mut Vec<BaoError>,
    ) -> Vec<SearchResult> {
        let mut found_symbols = vec![];

        found_symbols.reserve(symbols.len());

        for symbol in symbols {
            let name = &symbol.name;
            let (offset, va) = match symbol.find(data, self.image_base) {
                Err(e) => {
                    warnings.insert(0, e);
                    continue;
                }
                Ok(result) => result,
            };

            let (offset, index) = match self.get_section_data(offset, va) {
                None => {
                    warnings.insert(
                        0,
                        BaoError::BadPattern {
                            pattern: format!(
                                "{:?} ({:?}) could not be translated. Please check relative and \
                                 rip_relative!",
                                name, symbol.pattern
                            ),
                        },
                    );
                    continue;
                }
                Some(result) => result,
            };

            found_symbols.insert(
                0,
                SearchResult {
                    offset,
                    name: name.clone(),
                    index,
                },
            )
        }
        found_symbols
    }
}

impl<'a> Deref for BaoPE<'a> {
    type Target = PE<'a>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
