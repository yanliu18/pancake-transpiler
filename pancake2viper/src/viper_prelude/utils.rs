use viper::{AstFactory, Expr, Function, LocalVarDecl};

use crate::{
    ir::Model,
    utils::{EncodeOptions, EncodingMode, ViperUtils},
};

pub fn bound_bits_function(ast: AstFactory, bits: u64) -> Function {
    let x = ast.new_var("x", ast.int_type());
    let body = ast.and(
        ast.le_cmp(ast.int_zero(), x.1),
        ast.lt_cmp(
            x.1,
            ast.mul(ast.int_lit(4), ast.int_lit(2i64.pow(bits as u32 - 2))),
        ),
    );
    ast.function(
        &format!("bounded{}", bits),
        &[x.0],
        ast.bool_type(),
        &[],
        &[],
        ast.no_position(),
        Some(body),
    )
}

pub fn bound_bits_bv_function(ast: AstFactory, bits: u64) -> Function {
    let x = ast.new_var("x", ast.backend_bv64_type());
    let body = if bits == 64 {
        ast.true_lit()
    } else {
        ast.eq_cmp(
            ast.bv_and(x.1, ast.backend_bv64_lit(2u64.pow(bits as u32) - 1)),
            x.1,
        )
    };
    ast.function(
        &format!("boundedbv{}", bits),
        &[x.0],
        ast.bool_type(),
        &[],
        &[],
        ast.no_position(),
        Some(body),
    )
}

pub fn bound_function<'a>(
    ast: AstFactory<'a>,
    utils: &Utils,
    options: EncodeOptions,
    encoding_mode: EncodingMode,
) -> Function<'a> {
    let x = ast.new_var("x", ast.int_type());
    ast.function(
        "bounded",
        &[x.0],
        ast.bool_type(),
        &[],
        &[],
        ast.no_position(),
        Some(utils.bounded_f(x.1, options.word_size, EncodingMode::Int)),
    )
}

#[derive(Clone)]
pub struct Utils<'a> {
    ast: AstFactory<'a>,
    heap_typ: viper::Type<'a>,
    model: Model,
}

impl<'a> Utils<'a> {
    pub fn new(ast: AstFactory<'a>, heap_typ: viper::Type<'a>, model: Model) -> Self {
        Self {
            ast,
            heap_typ,
            model,
        }
    }

    pub fn bounded_f(&self, var: Expr, bits: u64, encoding_mode: EncodingMode) -> Expr<'a> {
        let func_name = match encoding_mode {
            EncodingMode::Int => format!("bounded{}", bits),
            EncodingMode::Bitvec => format!("boundedbv{}", bits),
        };
        self.ast.func_app(
            &func_name,
            &[var],
            self.ast.bool_type(),
            self.ast.no_position(),
        )
    }

    pub fn heap_var(&self) -> (LocalVarDecl<'a>, Expr<'a>) {
        self.ast.new_var("heap", self.heap_typ)
    }

    pub fn gv_ref(&self) -> (LocalVarDecl<'a>, Expr<'a>) {
        self.ast.new_var("gv", self.ast.ref_type())
    }

    pub fn heap_vars(&self) -> Vec<(LocalVarDecl<'a>, Expr<'a>)> {
        let heap_vars = vec![self.heap_var(), self.gv_ref()];
        heap_vars
    }

    pub fn get_model(&self) -> &Model {
        &self.model
    }
}
