use viper::{AstFactory, Expr, LocalVarDecl};

use crate::{
    ir::{self, Type},
    utils::EncodingMode,
};

use std::collections::HashSet;

use super::{
    errors::ToViperError, shape::Shape, EncodeOptions, Mangler, TranslationError, TypeContext,
    ViperEncodeCtx,
};

pub trait TryToIR {
    type Output;
    fn to_ir(self) -> Result<Self::Output, TranslationError>;
}

pub trait TryToIRGeneric<T> {
    fn to_ir(self) -> Result<T, TranslationError>;
}

pub trait Mangleable {
    fn mangle(&mut self, mangler: &mut Mangler) -> Result<(), TranslationError>;
}

pub trait TypeResolution {
    fn resolve_type(&self, is_annot: bool, ctx: &mut TypeContext) -> Result<(), TranslationError>;
}

pub trait ExprTypeResolution {
    fn resolve_expr_type(
        &self,
        is_annot: bool,
        ctx: &mut TypeContext,
    ) -> Result<Type, TranslationError>;
}

pub trait ForceToBool<'a> {
    type Output;
    fn force_to_bool(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError>;
}

pub trait TryToViper<'a> {
    type Output;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError>;
}

pub trait ToViper<'a> {
    type Output;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Self::Output;
    // fn to_viper_with_pos(
    //     &self,
    //     ctx: &mut ViperEncodeCtx<'a>,
    //     pos: viper::Position,
    // ) -> Self::Output {
    //     todo!()
    // }
}
pub trait ToViperType<'a> {
    fn to_viper_type(&self, ctx: &ViperEncodeCtx<'a>) -> viper::Type<'a>;
}

pub trait ToShape {
    fn to_shape(&self, ctx: &TypeContext) -> Shape;
}

pub trait TryToShape {
    fn to_shape(&self, ctx: &TypeContext) -> Result<Shape, TranslationError>;
}

pub trait TryToType {
    fn to_type(&self, ctx: &TypeContext) -> Result<ir::Type, TranslationError>;
}

pub trait ToType {
    fn to_type(&self, is_annot: bool) -> ir::Type;
}

pub trait ConstEvalExpr {
    fn const_eval(self, options: &EncodeOptions) -> ir::Expr;
}

pub trait ConstEval {
    fn const_eval(self, options: &EncodeOptions) -> Self;
}

pub trait ExprSubstitution {
    fn substitute(&mut self, old: &ir::Expr, new: &ir::Expr) -> bool;
}

pub trait MethodsCalled {
    fn methods_called(self) -> HashSet<String>;
}

pub trait ProgramToViper<'a> {
    fn to_viper(
        self,
        types: TypeContext,
        ast: AstFactory<'a>,
        options: EncodeOptions,
        encoding_mode: EncodingMode,
    ) -> Result<viper::Program<'a>, ToViperError>;
}

pub trait ViperUtils<'a> {
    fn new_var(&self, name: &str, typ: viper::Type) -> (LocalVarDecl<'a>, Expr<'a>);
    fn seq_slice(&self, seq: Expr<'a>, lower: Expr<'a>, upper: Expr<'a>) -> Expr<'a>;
    fn int_zero(&self) -> Expr<'a>;
    fn int_one(&self) -> Expr<'a>;
    fn int_two(&self) -> Expr<'a>;
    fn bv_zero(&self) -> Expr<'a>;
    fn bv_one(&self) -> Expr<'a>;
    fn bv_two(&self) -> Expr<'a>;
    fn bv_lit(&self, val: u64) -> Expr<'a>;
    fn bv_not(&self, a: Expr<'a>) -> Expr<'a>;
    fn bv_and(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_or(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_xor(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_add(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_sub(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_neg(&self, a: Expr<'a>) -> Expr<'a>;
    fn bv_mul(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_div(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_mod(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_ule(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_uge(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_ult(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_ugt(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_sle(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_sge(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_slt(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_sgt(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_shl(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_lshr(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
    fn bv_ashr(&self, a: Expr<'a>, b: Expr<'a>) -> Expr<'a>;
}
