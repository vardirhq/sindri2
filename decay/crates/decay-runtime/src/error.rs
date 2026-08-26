//! What stops a script, and what it says about why.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// A path was rooted at something the script holds that is not a reference,
    /// so there is nothing for the rest of the path to be about.
    NotAReference(String),
    /// A path was rooted at a reference that is empty. Reaching through nothing
    /// is a mistake worth naming, rather than silently doing nothing.
    NullReference(String),
    ContainerNotFound(String),
    FunctionNotFound(String),
    Arity {
        function: String,
        expected: usize,
        found: usize,
    },
    UnknownPath(String),
    Immutable(String),
    StackUnderflow,
    InvalidUnary,
    InvalidBinary,
    ExpectedBool,
    InvalidJump(usize),
    /// A script called deeper than [`Runtime::call_depth_limit`] allows.
    ///
    /// Without it, unbounded recursion overflowed the host's own stack and
    /// aborted the process — which, for a runtime meant to execute author
    /// scripts inside the editor, takes the editor and any unsaved work with
    /// it. A limit turns that into a value a caller can report.
    CallDepthExceeded {
        function: String,
        limit: usize,
    },
    Host(String),
}
