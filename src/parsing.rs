use crate::error::BaoError;
use clang::{CallingConvention, Entity, EntityKind, TranslationUnit, Type, TypeKind};
use pdb_wrapper::{pdb_meta::CallingConvention as CConv, PDBFunction, PDBType, StructField};
use std::{convert::TryFrom, ops::Deref};

pub(crate) struct BaoFunc {
    pub(crate) retn: PDBType,
    pub(crate) name: String,
    pub(crate) args: Vec<PDBType>,
    pub(crate) cconv: CConv,
}

impl Into<PDBFunction> for BaoFunc {
    fn into(self) -> PDBFunction {
        PDBFunction::new(self.retn, &self.args, self.cconv)
    }
}

#[derive(Debug)]
pub(crate) struct BaoType(PDBType);

impl Into<PDBType> for BaoType {
    fn into(self) -> PDBType {
        self.0
    }
}

#[derive(Debug)]
pub(crate) struct BaoStruct {
    pub(crate) name: String,
    pub(crate) fields: Vec<StructField>,
    pub(crate) size: usize,
}

impl<'tu> TryFrom<Entity<'tu>> for BaoStruct {
    type Error = BaoError;

    fn try_from(value: Entity<'tu>) -> Result<Self, Self::Error> {
        assert_eq!(value.get_kind(), EntityKind::StructDecl);

        let ty = value.get_type().ok_or(BaoError::InvalidName {
            ty: "struct".to_string(),
            name: format!("{:?}", value),
        })?;

        let name = value.get_display_name().ok_or(BaoError::InvalidName {
            ty: "struct".to_string(),
            name: format!("{:?}", value),
        })?;

        let fields = ty
            .get_fields()
            .ok_or(BaoError::InvalidStruct)?
            .into_iter()
            .map(|field| {
                let name = field.get_display_name().ok_or(BaoError::InvalidName {
                    ty: "field".to_string(),
                    name: format!("{:?}", field),
                })?;

                let field_type =
                    BaoType::try_from(field.get_type().ok_or(BaoError::InvalidField {
                        message: format!("{:?}", field.get_type()),
                    })?)?;

                let offset = ty
                    .get_offsetof(&name)
                    .map_err(|_| BaoError::InvalidOffset)?;
                // LLVM gives offsets in bits...
                assert_eq!(offset % 8, 0);
                Ok(StructField {
                    ty: field_type.into(),
                    name,
                    offset: (offset / 8) as u64,
                })
            })
            .collect::<Result<Vec<_>, BaoError>>()?;

        let size = ty.get_sizeof().map_err(|_| BaoError::InvalidStructSize)?;

        Ok(BaoStruct { name, fields, size })
    }
}

impl<'tu> TryFrom<Type<'tu>> for BaoType {
    type Error = BaoError;

    fn try_from(value: Type<'tu>) -> Result<Self, Self::Error> {
        use pdb_wrapper::pdb_meta::SimpleTypeKind;

        let kind = match value.get_kind() {
            TypeKind::Void => SimpleTypeKind::Void,
            // TODO: This might be kinda fucky depending on struct alignment...
            TypeKind::Bool => SimpleTypeKind::Boolean8,
            TypeKind::CharS => SimpleTypeKind::SignedCharacter,
            TypeKind::CharU => SimpleTypeKind::UnsignedCharacter,
            TypeKind::SChar => SimpleTypeKind::SignedCharacter,
            TypeKind::UChar => SimpleTypeKind::UnsignedCharacter,
            TypeKind::WChar => SimpleTypeKind::WideCharacter,
            TypeKind::Char16 => SimpleTypeKind::Character16,
            TypeKind::Char32 => SimpleTypeKind::Character32,
            TypeKind::Short => SimpleTypeKind::Int16Short,
            TypeKind::UShort => SimpleTypeKind::UInt16Short,
            TypeKind::Int => SimpleTypeKind::Int32,
            TypeKind::UInt => SimpleTypeKind::UInt32,
            TypeKind::Long => SimpleTypeKind::Int32Long,
            TypeKind::ULong => SimpleTypeKind::UInt32Long,
            TypeKind::LongLong => SimpleTypeKind::Int64Quad,
            TypeKind::ULongLong => SimpleTypeKind::UInt64Quad,
            TypeKind::Int128 => SimpleTypeKind::Int128,
            TypeKind::UInt128 => SimpleTypeKind::UInt128,
            TypeKind::Float16 => SimpleTypeKind::Float16,
            TypeKind::Float => SimpleTypeKind::Float32,
            TypeKind::Double => SimpleTypeKind::Float64,
            TypeKind::LongDouble => SimpleTypeKind::Float80,
            TypeKind::Float128 => SimpleTypeKind::Float128,
            _ => SimpleTypeKind::NotTranslated,
        };

        if kind == SimpleTypeKind::NotTranslated {
            if value.get_kind() == TypeKind::Typedef {
                return BaoType::try_from(value.get_canonical_type());
            }

            if value.get_kind() == TypeKind::ConstantArray {
                let target_type = value.get_element_type().ok_or(BaoError::TypeError {
                    message: format!(
                        "Couldn't get element type for {:?}",
                        value.get_display_name()
                    ),
                })?;
                let size = value.get_sizeof().map_err(|_| BaoError::TypeError {
                    message: format!("Couldn't get array size for {:?}", value.get_display_name()),
                })?;
                let target_type = BaoType::try_from(target_type)?;
                return Ok(BaoType(PDBType::ConstantArray(
                    Box::new(target_type.0),
                    size,
                )));
            }

            if value.get_kind() == TypeKind::FunctionPrototype {
                let func = BaoFunc::try_from(value)?.into();
                return Ok(BaoType(PDBType::Function(func)));
            }

            if value.get_kind() == TypeKind::Pointer {
                let target_type = value
                    .get_pointee_type()
                    .ok_or(BaoError::TypeError {
                        message: format!(
                            "Couldn't get pointee type for {:?}",
                            value.get_display_name()
                        ),
                    })?
                    .get_canonical_type();
                let target_type = BaoType::try_from(target_type)?;

                return Ok(BaoType(PDBType::Pointer(Box::new(target_type.0))));
            }

            if value.get_kind() == TypeKind::Record {
                let name = value
                    .get_declaration()
                    .and_then(|decl| decl.get_display_name())
                    .ok_or(BaoError::InvalidName {
                        ty: "type".to_string(),
                        name: format!("{:?}", value),
                    })?;

                return Ok(BaoType(PDBType::Struct(name)));
            }

            return Err(BaoError::UnknownType {
                name: format!("{:?}", value),
            });
        }
        Ok(BaoType(PDBType::SimpleType(kind)))
    }
}

impl<'tu> TryFrom<Type<'tu>> for BaoFunc {
    type Error = BaoError;

    fn try_from(value: Type<'tu>) -> Result<Self, Self::Error> {
        let retn =
            BaoType::try_from(value.get_result_type().ok_or(BaoError::InvalidRetnType {
                function: value.get_display_name(),
            })?)?
            .into();

        let args = value
            .get_argument_types()
            .ok_or(BaoError::InvalidFuncArgs {
                function: value.get_display_name(),
            })?
            .into_iter()
            .map(|ty| BaoType::try_from(ty))
            .map(|ty| ty.map(|ty| ty.into()))
            .collect::<Result<Vec<PDBType>, BaoError>>()?;

        let cconv = value
            .get_calling_convention()
            .and_then(|cconv| match cconv {
                CallingConvention::Cdecl => Some(CConv::NearC),
                CallingConvention::Fastcall => Some(CConv::NearFast),
                CallingConvention::Stdcall => Some(CConv::NearStdCall),
                CallingConvention::Thiscall => Some(CConv::ThisCall),
                CallingConvention::Win64 => Some(CConv::NearFast),
                _ => None,
            })
            .ok_or(BaoError::InvalidCConv {
                function: value.get_display_name(),
            })?;

        Ok(BaoFunc {
            retn,
            name: String::new(),
            args,
            cconv,
        })
    }
}

impl<'tu> TryFrom<Entity<'tu>> for BaoFunc {
    type Error = BaoError;

    fn try_from(value: Entity<'tu>) -> Result<Self, Self::Error> {
        assert_eq!(value.get_kind(), EntityKind::FunctionDecl);
        let name = value.get_name().ok_or(BaoError::InvalidFuncName {
            location: format!("{:?}", value.get_location()),
        })?;

        let func_ty = value.get_type().ok_or(BaoError::InvalidFunc {
            location: format!("{:?}", value.get_display_name()),
        })?;

        let mut func = BaoFunc::try_from(func_ty)?;
        func.name = name;

        Ok(func)
    }
}

pub(crate) struct BaoTU<'tu>(TranslationUnit<'tu>);

impl<'tu> BaoTU<'tu> {
    pub fn has_errors(&self) -> bool {
        !self.0.get_diagnostics().is_empty()
    }

    pub fn get_entities(&self, kind: EntityKind) -> Vec<Entity> {
        self.0
            .get_entity()
            .get_children()
            .into_iter()
            .filter(|e| e.get_kind() == kind)
            .filter(|e| !e.is_in_system_header())
            .collect::<Vec<_>>()
    }
}

impl<'tu> Deref for BaoTU<'tu> {
    type Target = TranslationUnit<'tu>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'tu> From<TranslationUnit<'tu>> for BaoTU<'tu> {
    fn from(tu: TranslationUnit<'tu>) -> Self {
        BaoTU(tu)
    }
}
