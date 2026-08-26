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
    /// A script ran longer than [`Runtime::operation_budget`] allows.
    ///
    /// The call-depth limit bounds recursion, which was the only way to run
    /// forever before loops existed. `while` removed that guarantee: a loop
    /// that never ends uses no extra stack and would simply never return, which
    /// inside the editor means a frame that never finishes. The budget is what
    /// makes a loop safe to offer at all — it turns a runaway script into a
    /// reported failure for one entity, the way every other runtime error is.
    OperationBudgetExceeded {
        limit: usize,
    },
    Host(String),
}
