use crate::{
    error::BaoError,
    parsing::{BaoFunc, BaoStruct},
};
use clang::{Clang, Entity, EntityKind, Index};

use crate::{
    matching::BaoConfiguration,
    parsing::{BaoTU, BaoType},
    pe::BaoPE,
};
use clap::{App, Arg};
use log::{error, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode};
use std::{collections::HashMap, convert::TryFrom, error::Error};

mod error;
mod matching;
mod parsing;
mod pe;

pub fn main() -> Result<(), Box<dyn Error>> {
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,
        Config::default(),
        TerminalMode::Mixed,
    )])?;

    let matches = App::new("bao")
        .version(env!("CARGO_PKG_VERSION"))
        .author("wlan <not-wlan@protonmail.com>")
        .about("Parse C to PDBs")
        .arg(
            Arg::with_name("config")
                .short("c")
                .long("config")
                .value_name("CONFIG")
                .help("Sets a custom config file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("output")
                .short("o")
                .long("output")
                .value_name("OUTPUT")
                .help("Sets a custom output file")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("BINARY")
                .help("Sets the input binary file to use")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("SOURCE")
                .help("Sets the input source code file to use")
                .required(true)
                .index(2),
        )
        .get_matches();

    // Unwrapping these is fine since they're marked as required.
    let path = matches.value_of("BINARY").unwrap();
    let source = matches.value_of("SOURCE").unwrap();

    // Default to appending .pdb to the input path if no other path is specified
    let output = matches
        .value_of("output")
        .map(|output| output.to_string())
        .unwrap_or(format!("{}.pdb", path));

    let raw_pe = std::fs::read(&path)?;
    let pe = BaoPE::from(goblin::pe::PE::parse(&raw_pe)?);

    // Load configuration or default to empty configuration. This will still add
    // structures and function prototypes to the PDB.
    let config = match matches.value_of("config") {
        None => Ok(BaoConfiguration::default()),
        Some(config) => {
            let config = std::fs::read_to_string(config)?;
            serde_json::from_str(&config)
        }
    }?;

    info!(
        "Loaded {} functions and {} globals.",
        config.functions.len(),
        config.globals.len()
    );

    // This needs to happen (on Linux atleast), otherwise clang won't load.
    clang_sys::load()?;

    let clang = Clang::new()?;
    let index = Index::new(&clang, false, false);

    let tu = BaoTU::from(
        index
            .parser(source)
            .arguments(if pe.is_64 {
                &["-Werror"]
            } else {
                &["-m32", "-Werror"]
            })
            .parse()?,
    );

    let mut generated = pdb_wrapper::PDB::new(false)?;

    if tu.has_errors() {
        for error in tu.get_diagnostics() {
            error!("{}", error);
        }
        info!("Please fix these errors before continuing!");
        return Ok(());
    }

    let funcs = tu.get_entities(EntityKind::FunctionDecl);
    let structs = tu.get_entities(EntityKind::StructDecl);
    let globals = tu.get_entities(EntityKind::VarDecl);

    let mut warnings = vec![];

    info!("Parsed {} function definitions.", funcs.len());
    info!("Parsed {} struct definitions.", structs.len());
    info!("Parsed {} global variable definitions.", globals.len());

    // Parse structs first so they may be used by global variables and functions.
    structs
        .into_iter()
        .map(|strct: Entity| -> Result<(), BaoError> {
            let BaoStruct { name, fields, size } = BaoStruct::try_from(strct)?;
            Ok(generated.insert_struct(&name, &fields, size as u64)?)
        })
        .collect::<Result<Vec<_>, BaoError>>()?;

    // Pre-process the function types to allow the creation of function types that
    // aren't included in a pattern.
    let func_types = funcs
        .into_iter()
        .map(|func| BaoFunc::try_from(func))
        .collect::<Result<Vec<_>, BaoError>>()?
        .into_iter()
        .map(|func| {
            Ok((
                func.name.clone(),
                generated.insert_function_metadata(&func.into(), "")?,
            ))
        })
        .collect::<Result<HashMap<_, _>, BaoError>>()?;

    // Insert the functions into the PDB using our function name -> type lookup
    // table.
    pe.find_symbols(config.functions, &raw_pe, &mut warnings)
        .into_iter()
        .map(|result| (func_types.get(&result.name), result))
        .map(|(ty, result)| {
            generated
                .insert_function(result.index, result.offset, &result.name, ty.cloned())
                .map_err(|e| BaoError::from(e))
        })
        .collect::<Result<_, BaoError>>()?;

    // Pre-process global variables to `BaoType`. This way we can just call get on
    // the HashMap and don't have to lazily evaluate the code.
    let globals = globals
        .into_iter()
        .filter_map(|global| global.get_display_name().map(|name| (name, global)))
        .filter_map(|(name, var)| var.get_type().map(|ty| (name, ty)))
        .map(|(name, ty)| BaoType::try_from(ty).map(|ty| (name, ty.into())))
        .collect::<Result<HashMap<_, _>, BaoError>>()?;

    // Insert the global variables with types, if they're specified.
    pe.find_symbols(config.globals, &raw_pe, &mut warnings)
        .into_iter()
        .map(|result| (globals.get(&result.name), result))
        .map(|(ty, result)| {
            generated
                .insert_global(&result.name, result.index, result.offset, ty)
                .map_err(|e| BaoError::from(e))
        })
        .collect::<Result<_, BaoError>>()?;

    // Inform the user about warnings that may have occured during the pattern
    // matching procedure. These warnings are non-critical and shouldn't lead to
    // a panic.
    warnings.into_iter().for_each(|err| warn!("{}", err));

    // Finally, save the generated PDB to the path we calculated in the beginning
    generated.commit(&path, &output)?;
    Ok(())
}
