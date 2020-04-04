use clang::SourceError;
use pdb_wrapper::Error as PDBWrapperError;
use snafu::Snafu;

#[derive(Debug, Snafu)]
pub enum BaoError {
    #[snafu(display("Bad pattern detected! {}", pattern))]
    BadPattern {
        pattern: String,
    },
    #[snafu(display("Pattern {:?} was not found!", name))]
    PatternNotFound {
        name: String,
    },
    #[snafu(display("Invalid function name: {:#?}", location))]
    InvalidFuncName {
        location: String,
    },
    #[snafu(display("Invalid function: {:#?}", location))]
    InvalidFunc {
        location: String,
    },
    #[snafu(display("Invalid function args: {:#?}", function))]
    InvalidFuncArgs {
        function: String,
    },
    #[snafu(display("Invalid return type for {:#?}", function))]
    InvalidRetnType {
        function: String,
    },
    #[snafu(display("Invalid calling convention for {:#?}", function))]
    InvalidCConv {
        function: String,
    },
    #[snafu(display("Invalid {} name for {}", ty, name))]
    InvalidName {
        ty: String,
        name: String,
    },
    #[snafu(display("Unknown type: {:#?}", name))]
    UnknownType {
        name: String,
    },
    InvalidStruct,
    InvalidStructSize,
    InvalidOffset,
    #[snafu(display("Error during type translation: {:#?}", message))]
    TypeError {
        message: String,
    },
    #[snafu(display("Invalid struct field: {:#?}", message))]
    InvalidField {
        message: String,
    },
    #[snafu(display("Error during parsing: {:#?}", message))]
    ClangError {
        message: String,
    },
    #[snafu(display("Error during PDB generation: {:#?}", message))]
    PDBError {
        message: String,
    },
}

impl From<PDBWrapperError> for BaoError {
    fn from(e: PDBWrapperError) -> Self {
        BaoError::PDBError {
            message: format!("{:?}", e),
        }
    }
}

impl From<SourceError> for BaoError {
    fn from(e: SourceError) -> Self {
        BaoError::ClangError {
            message: e.to_string(),
        }
    }
}
