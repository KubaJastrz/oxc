use oxc_allocator::Allocator;

use crate::ast::Program;

pub mod ancestor;
pub use ancestor::Ancestor;
mod context;
pub use context::{FinderRet, TraverseCtx};
#[allow(clippy::module_inception)]
mod traverse;
pub use traverse::Traverse;
mod walk;

pub fn traverse_mut<'a, Tr: Traverse<'a>>(
    traverser: &mut Tr,
    program: &mut Program<'a>,
    allocator: &'a Allocator,
) {
    let mut ctx = TraverseCtx::new(allocator);
    walk::walk_program(traverser, program, &mut ctx);
    debug_assert!(ctx.stack_is_empty());
}
