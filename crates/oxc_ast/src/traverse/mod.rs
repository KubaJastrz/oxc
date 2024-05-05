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

#[allow(unsafe_code)]
pub fn traverse_mut<'a, Tr: Traverse<'a>>(
    traverser: &mut Tr,
    program: &mut Program<'a>,
    allocator: &'a Allocator,
) {
    let mut ctx = TraverseCtx::new(allocator);
    // SAFETY: Walk functions are constructed to avoid unsoundness
    unsafe { walk::walk_program(traverser, program as *mut Program, &mut ctx) };
    debug_assert!(ctx.stack_is_empty());
}
