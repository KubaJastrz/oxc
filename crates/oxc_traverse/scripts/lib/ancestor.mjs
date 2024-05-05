import {camelToSnake, snakeToCamel} from './utils.mjs';

export default function generateAncestorsCode(types) {
    const variantNamesForEnums = Object.create(null);
    let enumVariants = '',
        isFunctions = '',
        ancestorTypes = '',
        discriminant = 1;
    for (const type of Object.values(types)) {
        if (type.kind === 'enum') continue;

        // TODO: Don't create `Ancestor`s for types which are never a parent
        // e.g. `IdentifierReference`
        const typeSnakeName = camelToSnake(type.name),
            typeScreamingName = typeSnakeName.toUpperCase();
        for (const field of type.fields) {
            const offsetVarName = `OFFSET_${typeScreamingName}_${field.name.toUpperCase()}`;
            field.offsetVarName = offsetVarName;
            ancestorTypes += `pub(crate) const ${offsetVarName}: usize = offset_of!(${type.name}, ${field.rawName});\n`;
        }

        const variantNames = [];
        for (const field of type.fields) {
            const fieldTypeName = field.innerTypeName,
                fieldType = types[fieldTypeName];
            if (!fieldType) continue;

            let methodsCode = '';
            for (const otherField of type.fields) {
                if (otherField === field) continue;

                methodsCode += `
                    #[inline]
                    pub fn ${otherField.rawName}(&self) -> &${otherField.rawTypeName} {
                        unsafe {
                            &*(
                                (self.0 as *const u8).add(${otherField.offsetVarName})
                                as *const ${otherField.rawTypeName}
                            )
                        }
                    }
                `;
            }

            const fieldNameCamel = snakeToCamel(field.name),
                lifetime = type.hasLifetime ? "<'a>" : '',
                structName = `${type.name}Without${fieldNameCamel}${lifetime}`;

            ancestorTypes += `
                #[repr(transparent)]
                #[derive(Debug)]
                pub struct ${structName}(
                    pub(crate) *const ${type.name}${lifetime}
                );

                impl${lifetime} ${structName} {
                    ${methodsCode}
                }
            `;

            const variantName = `${type.name}${fieldNameCamel}`;
            variantNames.push(variantName);

            enumVariants += `${variantName}(${structName}) = ${discriminant},\n`;
            field.ancestorDiscriminant = discriminant;
            discriminant++;

            if (fieldType.kind === 'enum') {
                (variantNamesForEnums[fieldTypeName] || (variantNamesForEnums[fieldTypeName] = []))
                    .push(variantName);
            }
        }

        if (variantNames.length > 0) {
            isFunctions += `
                #[inline]
                pub fn is_${typeSnakeName}(&self) -> bool {
                    matches!(self, ${variantNames.map(name => `Self::${name}(_)`).join(' | ')})
                }
            `;
        }
    }

    for (const [typeName, variantNames] of Object.entries(variantNamesForEnums)) {
        isFunctions += `
            #[inline]
            pub fn is_via_${camelToSnake(typeName)}(&self) -> bool {
                matches!(self, ${variantNames.map(name => `Self::${name}(_)`).join(' | ')})
            }
        `;
    }

    const discriminantType = discriminant <= 256 ? 'u8' : 'u16';

    return `
        #![allow(
            unsafe_code,
            clippy::missing_safety_doc,
            clippy::ptr_as_ptr,
            clippy::undocumented_unsafe_blocks,
            clippy::cast_ptr_alignment
        )]

        // TODO: Remove unneeded offset consts, then remove next line
        #![allow(dead_code)]

        use memoffset::offset_of;

        use oxc_allocator::{Box, Vec};
        #[allow(clippy::wildcard_imports)]
        use oxc_ast::ast::*;
        use oxc_span::{Atom, SourceType, Span};
        use oxc_syntax::operator::{
            AssignmentOperator, BinaryOperator, LogicalOperator, UnaryOperator, UpdateOperator,
        };

        pub(crate) type AncestorDiscriminant = ${discriminantType};

        /// Ancestor type used in AST traversal.
        ///
        /// Encodes both the type of the parent, and child's location in the parent.
        /// i.e. variants for \`BinaryExpressionLeft\` and \`BinaryExpressionRight\`, not just \`BinaryExpression\`.
        #[repr(C, ${discriminantType})]
        #[derive(Debug)]
        pub enum Ancestor<'a> {
            None = 0,
            ${enumVariants}
        }

        impl<'a> Ancestor<'a> {
            ${isFunctions}
        }

        ${ancestorTypes}
    `;
}
