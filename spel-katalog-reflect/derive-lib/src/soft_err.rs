//! Add errors that will be reported without stopping parsing.

use ::core::{cell::RefCell, mem};

use ::proc_macro2::TokenStream;

thread_local! {
    static SOFT_ERR_STACK: RefCell<Vec<::syn::Error>> = const { RefCell::new(Vec::new()) };
}

/// Push an error to soft error stack.
pub fn push_soft_err(err: ::syn::Error) {
    SOFT_ERR_STACK.with(|stack| stack.borrow_mut().push(err));
}

/// Push an error to soft error stack.
pub fn with_soft_err_stack(f: impl FnOnce() -> TokenStream) -> TokenStream {
    let mut backup = Vec::new();
    SOFT_ERR_STACK.with(|stack| mem::swap(&mut backup, &mut *stack.borrow_mut()));
    let mut tokens = f();
    SOFT_ERR_STACK.with(|stack| mem::swap(&mut backup, &mut *stack.borrow_mut()));

    for err in backup {
        tokens.extend(err.into_compile_error());
    }

    tokens
}
