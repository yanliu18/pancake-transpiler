use crate::ir::{Expr, Expr::*, Stmt, Stmt::*};
use crate::utils::MethodsCalled;
use std::collections::HashSet;

impl MethodsCalled for Expr {
    fn methods_called(self) -> HashSet<String> {
        // it is okay to not recurse here because the only way this can come
        // about in Pancake is as a method call assigned to a variable
        match self {
            MethodCall(e) => [e.fname.to_owned()].to_vec().into_iter().collect(),
            _ => HashSet::new(),
        }
    }
}

impl MethodsCalled for Stmt {
    fn methods_called(self) -> HashSet<String> {
        match self {
            Assign(s) => s.rhs.methods_called(),
            Definition(s) => &(*s.scope).methods_called() | &s.rhs.methods_called(),
            Call(s) => s.call.methods_called(),
            Seq(s) => s
                .stmts
                .into_iter()
                .map(|e| e.methods_called())
                .flat_map(|s| s)
                .collect(),
            If(s) => &(*s.if_branch).methods_called() | &(*s.else_branch).methods_called(),
            While(s) => (*s.body).methods_called(),
            _ => HashSet::new(),
        }
    }
}
