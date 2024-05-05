import assert from 'assert';
import {typeAndWrappers, unwrapTypeName, toTypeName, camelToSnake, snakeToCamel} from './utils.mjs';

export default function generateWalkFunctionsCode(types) {
    let walkMethods = '';
    for (const type of Object.values(types)) {
        if (type.kind === 'struct') {
            walkMethods += generateWalkForStruct(type, types);
        } else {
            walkMethods += generateWalkForEnum(type, types);
        }
    }

    return `
        #![allow(
            unsafe_code,
            clippy::missing_safety_doc,
            clippy::missing_panics_doc,
            clippy::undocumented_unsafe_blocks,
            clippy::semicolon_if_nothing_returned,
            clippy::ptr_as_ptr,
            clippy::borrow_as_ptr,
            clippy::cast_ptr_alignment
        )]

        use oxc_allocator::Vec;
        #[allow(clippy::wildcard_imports)]
        use oxc_ast::ast::*;

        use crate::{ancestor, Ancestor, Traverse, TraverseCtx};

        ${walkMethods}

        pub(crate) unsafe fn walk_statements<'a, Tr: Traverse<'a>>(
            traverser: &mut Tr,
            stmts: *mut Vec<'a, Statement<'a>>,
            ctx: &mut TraverseCtx<'a>
        ) {
            traverser.enter_statements(&mut *stmts, ctx);
            for stmt in (*stmts).iter_mut() {
                walk_statement(traverser, stmt, ctx);
            }
            traverser.exit_statements(&mut *stmts, ctx);
        }
    `;
}

function generateWalkForStruct(type, types) {
    const visitedFields = type.fields.filter(field => unwrapTypeName(field.type) in types);

    const fieldsCodes = visitedFields.map((field, index) => {
        const {name: fieldTypeName, wrappers: fieldTypeWrappers} = typeAndWrappers(field.type);

        const retagCode = index === 0 ? '' : `ctx.retag_stack(${field.ancestorDiscriminant});`,
            fieldCode = `(node as *mut u8).add(ancestor::${field.offsetVarName}) as *mut ${field.type}`;

        if (fieldTypeWrappers[0] === 'Option') {
            const remainingWrappers = fieldTypeWrappers.slice(1);

            let walkCode;
            if (remainingWrappers.length === 1 && remainingWrappers[0] === 'Vec') {
                if (fieldTypeName === 'Statement') {
                    // Special case for `Option<Vec<Statement>>`
                    walkCode = `walk_statements(traverser, field as *mut _, ctx);`;
                } else {
                    walkCode = `
                        for item in field.iter_mut() {
                            walk_${camelToSnake(fieldTypeName)}(traverser, item as *mut _, ctx);
                        }
                    `.trim();
                }
            } else if (remainingWrappers.length === 1 && remainingWrappers[0] === 'Box') {
                walkCode = `walk_${camelToSnake(fieldTypeName)}(traverser, (&mut **field) as *mut _, ctx);`;
            } else {
                assert(remainingWrappers.length === 0, `Cannot handle struct field with type ${field.type}`);
                walkCode = `walk_${camelToSnake(fieldTypeName)}(traverser, field as *mut _, ctx);`;
            }

            return `
                if let Some(field) = &mut *(${fieldCode}) {
                    ${retagCode}
                    ${walkCode}
                }
            `;
        }

        if (fieldTypeWrappers[0] === 'Vec') {
            const remainingWrappers = fieldTypeWrappers.slice(1);

            let walkVecCode;
            if (remainingWrappers.length === 0 && fieldTypeName === 'Statement') {
                // Special case for `Vec<Statement>`
                walkVecCode = `walk_statements(traverser, ${fieldCode}, ctx);`
            } else {
                let walkCode = `walk_${camelToSnake(fieldTypeName)}(traverser, item as *mut _, ctx);`,
                    iterModifier = '';
                if (remainingWrappers.length === 1 && remainingWrappers[0] === 'Option') {
                    iterModifier = '.flatten()';
                } else {
                    assert(
                        remainingWrappers.length === 0,
                        `Cannot handle struct field with type ${field.type}`
                    );
                }
                walkVecCode = `
                    for item in (*(${fieldCode})).iter_mut()${iterModifier} {
                        ${walkCode}
                    }
                `.trim();
            }

            return `
                ${retagCode}
                ${walkVecCode}
            `;
        }

        if (fieldTypeWrappers.length === 1 && fieldTypeWrappers[0] === 'Box') {
            return `
                ${retagCode}
                walk_${camelToSnake(fieldTypeName)}(
                    traverser, (&mut **(${fieldCode})) as *mut _, ctx
                );
            `;
        }

        assert(fieldTypeWrappers.length === 0, `Cannot handle struct field with type: ${field.type}`);

        return `
            ${retagCode}
            walk_${camelToSnake(fieldTypeName)}(traverser, ${fieldCode}, ctx);
        `;
    });

    if (visitedFields.length > 0) {
        const field = visitedFields[0],
            fieldCamelName = snakeToCamel(field.name);
        fieldsCodes.unshift(`
            ctx.push_stack(
                Ancestor::${type.name}${fieldCamelName}(
                    ancestor::${type.name}Without${fieldCamelName}(node)
                )
            );
        `);
        fieldsCodes.push('ctx.pop_stack();');
    }

    const snakeName = camelToSnake(type.name);
    return `
        pub(crate) unsafe fn walk_${snakeName}<'a, Tr: Traverse<'a>>(
            traverser: &mut Tr,
            node: *mut ${toTypeName(type)},
            ctx: &mut TraverseCtx<'a>
        ) {
            traverser.enter_${snakeName}(&mut *node, ctx);
            ${fieldsCodes.join('\n')}
            traverser.exit_${snakeName}(&mut *node, ctx);
        }
    `.replace(/\n\s*\n+/g, '\n');
}

function generateWalkForEnum(type, types) {
    const variantCodes = type.variants.map((variant) => {
        const {name: variantTypeName, wrappers: fieldTypeWrappers} = typeAndWrappers(variant.type),
            variantType = types[variantTypeName];
        assert(variantType, `Cannot handle enum variant with type: ${variant.type}`);

        let nodeCode = 'node';
        if (fieldTypeWrappers.length === 1 && fieldTypeWrappers[0] === 'Box') {
            nodeCode = '(&mut **node)';
        } else {
            assert(fieldTypeWrappers.length === 0, `Cannot handle enum variant with type: ${variant.type}`);
        }

        return `${type.name}::${variant.name}(node) => `
            + `walk_${camelToSnake(variantTypeName)}(traverser, ${nodeCode} as *mut _, ctx),`;
    });

    const missingVariants = [];
    for (const inheritedTypeName of type.inherits) {
        // Recurse into nested inherited types
        const variantMatches = [],
            inheritedFrom = [inheritedTypeName];
        for (let i = 0; i < inheritedFrom.length; i++) {
            const inheritedTypeName = inheritedFrom[i],
                inheritedType = types[inheritedTypeName];
            if (!inheritedType || inheritedType.kind !== 'enum') {
                missingVariants.push(inheritedTypeName);
            } else {
                variantMatches.push(...inheritedType.variants.map(
                    variant => `${type.name}::${variant.name}(_)`
                ));
                inheritedFrom.push(...inheritedType.inherits);
            }
        }

        variantCodes.push(
            `${variantMatches.join(' | ')} => `
            + `walk_${camelToSnake(inheritedTypeName)}(traverser, node as *mut _, ctx),`
        );
    }

    assert(missingVariants.length === 0, `Missing enum variants: ${missingVariants.join(', ')}`);

    const snakeName = camelToSnake(type.name);
    return `
        pub(crate) unsafe fn walk_${snakeName}<'a, Tr: Traverse<'a>>(
            traverser: &mut Tr,
            node: *mut ${toTypeName(type)},
            ctx: &mut TraverseCtx<'a>
        ) {
            traverser.enter_${snakeName}(&mut *node, ctx);
            match &mut *node {
                ${variantCodes.join('\n')}
            }
            traverser.exit_${snakeName}(&mut *node, ctx);
        }
    `;
}
