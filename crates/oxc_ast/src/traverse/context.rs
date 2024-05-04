use oxc_allocator::Allocator;

use super::Ancestor;
use crate::AstBuilder;

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
        Self { stack: Vec::new(), ast: AstBuilder::new(allocator) }
    }

    /// Allocate a node in the arena.
    /// Returns a `Box<T>`.
    #[inline]
    pub fn alloc<T>(&self, node: T) -> oxc_allocator::Box<'a, T> {
        self.ast.alloc(node)
    }

    /// Get parent of current node.
    /// # Panics
    /// Panics if no parent (i.e. called when visiting `Program`).
    #[inline]
    pub fn parent(&self) -> &Ancestor<'a> {
        // TODO: Would be better to make `Ancestor` `Copy` and return an owned `Ancestor`
        // for this function and also `ancestor` and `find_ancestor`, but Miri doesn't like it
        self.stack.last().unwrap()
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

    /// Replace last item on stack.
    /// # SAFETY
    /// Stack must not be empty.
    #[inline]
    #[allow(unsafe_code)]
    pub(super) unsafe fn replace_stack(&mut self, ancestor: Ancestor<'a>) {
        *self.stack.last_mut().unwrap_unchecked() = ancestor;
    }

    /// Return if stack and stack arena are empty
    pub(super) fn stack_is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

pub enum FinderRet<T> {
    Found(T),
    Stop,
    Continue,
}
