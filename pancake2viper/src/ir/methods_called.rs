use crate::ir::{Expr, Expr::*, Stmt, Stmt::*};
use crate::utils::{FunctionsUsed, MethodsCalled};
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
            ExtCall(s) => s
                .args
                .into_iter()
                .flat_map(|a| a.methods_called())
                .collect(),
            Seq(s) => s
                .stmts
                .into_iter()
                .flat_map(|e| e.methods_called())
                .collect(),
            If(s) => &(*s.if_branch).methods_called() | &(*s.else_branch).methods_called(),
            While(s) => (*s.body).methods_called(),
            Store(s) => &s.address.methods_called() | &s.value.methods_called(),
            StoreBits(s) => &s.address.methods_called() | &s.value.methods_called(),
            SharedStore(s) => &s.address.methods_called() | &s.value.methods_called(),
            SharedStoreBits(s) => &s.address.methods_called() | &s.value.methods_called(),
            SharedLoad(s) => &s.address.methods_called() | &s.dst.methods_called(),
            SharedLoadBits(s) => &s.address.methods_called() | &s.dst.methods_called(),
            _ => HashSet::new(),
        }
    }
}

impl FunctionsUsed for Expr {
    fn functions_used(&self) -> HashSet<String> {
        match self {
            BinOp(b) => &b.left.functions_used() | &b.right.functions_used(),
            UnOp(u) => u.right.functions_used(),
            Quantified(q) => q.body.functions_used(),
            Ternary(t) => {
                &(&t.cond.functions_used() | &t.left.functions_used()) | &t.right.functions_used()
            }
            AccessPredicate(a) => a.field.functions_used(),
            AccessSlice(s) => {
                &(&s.field.functions_used() | &s.lower.functions_used()) | &s.upper.functions_used()
            }
            Old(o) => o.expr.functions_used(),
            SeqLength(s) => s.expr.functions_used(),
            Contains(c) => &c.left.functions_used() | &c.right.functions_used(),
            ViperFieldAccess(v) => v.obj.functions_used(),
            FunctionCall(f) => HashSet::from([f.fname.to_owned()]),
            _ => HashSet::new(),
        }
    }
}

impl FunctionsUsed for Stmt {
    fn functions_used(&self) -> HashSet<String> {
        match self {
            Assign(s) => s.rhs.functions_used(),
            Definition(s) => &(*s.scope).functions_used() | &s.rhs.functions_used(),
            Call(s) => s.call.functions_used(),
            ExtCall(s) => s
                .args
                .clone()
                .iter()
                .flat_map(|a| a.functions_used())
                .collect(),
            Seq(s) => s
                .stmts
                .clone()
                .iter()
                .flat_map(|e| e.functions_used())
                .collect(),
            If(s) => &(*s.if_branch).functions_used() | &(*s.else_branch).functions_used(),
            While(s) => (*s.body).functions_used(),
            Store(s) => &s.address.functions_used() | &s.value.functions_used(),
            StoreBits(s) => &s.address.functions_used() | &s.value.functions_used(),
            SharedStore(s) => &s.address.functions_used() | &s.value.functions_used(),
            SharedStoreBits(s) => &s.address.functions_used() | &s.value.functions_used(),
            SharedLoad(s) => &s.address.functions_used() | &s.dst.functions_used(),
            SharedLoadBits(s) => &s.address.functions_used() | &s.dst.functions_used(),
            _ => HashSet::new(),
        }
    }
}
