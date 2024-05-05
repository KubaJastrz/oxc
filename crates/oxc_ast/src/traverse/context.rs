use oxc_allocator::Allocator;

use super::Ancestor;
use crate::AstBuilder;

const INITIAL_STACK_CAPACITY: usize = 64;

/// Traverse context.
///
/// Passed to all AST visitor functions.
///
/// Provides ability to:
/// * Query parent/ancestor of current node.
/// * Create AST nodes via `ctx.ast`.
/// * Allocate into arena via `ctx.alloc()`.
pub struct TraverseCtx<'a> {
    stack: Vec<Ancestor<'a>>,
    pub ast: AstBuilder<'a>,
}

impl<'a> TraverseCtx<'a> {
    /// Create new traversal context.
    pub fn new(allocator: &'a Allocator) -> Self {
        let mut stack = Vec::with_capacity(INITIAL_STACK_CAPACITY);
        stack.push(Ancestor::None);
        Self { stack, ast: AstBuilder::new(allocator) }
    }

    /// Allocate a node in the arena.
    /// Returns a `Box<T>`.
    #[inline]
    pub fn alloc<T>(&self, node: T) -> oxc_allocator::Box<'a, T> {
        self.ast.alloc(node)
    }

    /// Get parent of current node.
    #[inline]
    #[allow(unsafe_code)]
    pub fn parent(&self) -> &Ancestor<'a> {
        // SAFETY: Stack contains 1 entry initially. Entries are pushed as traverse down the AST,
        // and popped as go back up. So even when visiting `Program`, the initial entry is in the stack.
        unsafe { self.stack.last().unwrap_unchecked() }
    }

    /// Get ancestor of current node.
    /// `level` is number of levels above.
    /// `ancestor(1).unwrap()` is equivalent to `parent()`.
    #[inline]
    pub fn ancestor(&self, level: usize) -> Option<&Ancestor<'a>> {
        self.stack.get(self.stack.len() - level)
    }

    /// Walk up trail of ancestors to find a node.
    ///
    /// `finder` should return:
    /// * `FinderRet::Found(value)` to stop walking and return `Some(value)`.
    /// * `FinderRet::Stop` to stop walking and return `None`
    /// * `FinderRet::Continue` to continue walking up.
    pub fn find_ancestor<F, O>(&self, finder: F) -> Option<O>
    where
        F: Fn(&Ancestor<'a>) -> FinderRet<O>,
    {
        for ancestor in self.stack.iter().rev() {
            match finder(ancestor) {
                FinderRet::Found(res) => return Some(res),
                FinderRet::Stop => return None,
                FinderRet::Continue => {}
            }
        }
        None
    }

    /// Get depth of ancestry stack.
    /// i.e. How many nodes above this one in the tree.
    #[inline]
    pub fn ancestors_depth(&self) -> usize {
        self.stack.len()
    }

    /// Push item onto stack.
    #[inline]
    pub(super) fn push_stack(&mut self, ancestor: Ancestor<'a>) {
        self.stack.push(ancestor);
    }

    /// Pop last item off stack.
    /// # SAFETY
    /// * Stack must not be empty.
    /// * Each `pop_stack` call must correspond to a `push_stack` call for same type.
    #[inline]
    #[allow(unsafe_code)]
    pub(super) unsafe fn pop_stack(&mut self) {
        self.stack.pop().unwrap_unchecked();
    }

    /// Retag last item on stack.
    /// # SAFETY
    /// * Stack must not be empty.
    /// * Last item on stack must contain type corresponding to provided discriminant.
    #[inline]
    #[allow(unsafe_code, clippy::ptr_as_ptr, clippy::ref_as_ptr)]
    pub(super) unsafe fn retag_stack(&mut self, discriminant: u16) {
        *(self.stack.last_mut().unwrap_unchecked() as *mut _ as *mut u16) = discriminant;
    }
}

pub enum FinderRet<T> {
    Found(T),
    Stop,
    Continue,
}
