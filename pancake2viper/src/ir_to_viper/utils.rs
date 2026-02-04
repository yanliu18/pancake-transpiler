use viper::BvSize::BV64;
use viper::{AstFactory, Expr};

use crate::ir::{ShiftType, UnOpType};
use crate::{
    ir::{self, Arg, BinOpType, FnDec, Type},
    utils::{
        EncodingMode, ToViper, ToViperError, ToViperType, TryToViper, ViperEncodeCtx, ViperUtils,
    },
};

impl<'a, T: TryToViper<'a>> TryToViper<'a> for Vec<T> {
    type Output = Vec<T::Output>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        self.into_iter()
            .map(|a| a.to_viper(ctx))
            .collect::<Result<Vec<_>, _>>()
    }
}

impl<'a, T: ToViper<'a>> ToViper<'a> for Vec<T> {
    type Output = Vec<T::Output>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Self::Output {
        self.into_iter().map(|a| a.to_viper(ctx)).collect()
    }
}

impl Arg {
    /// Generates preconditions for an argument
    ///
    /// If an `Arg` is not of shape `1` it is encoded as a `Seq[Int]`.
    /// We can automatically infer the length of the sequence given we know its shape.
    /// We also assert that all the elements of the sequence or the single Int are bounded
    pub fn precondition<'a>(
        &self,
        is_predicate: bool,
        ctx: &ViperEncodeCtx<'a>,
    ) -> Option<Expr<'a>> {
        match ctx.encoding_mode {
            EncodingMode::Int => {
                let ast = ctx.ast;
                let arg_var = ctx.ast.new_var(&self.name, self.typ.to_viper_type(ctx)).1;

                match &self.typ {
                    ir::Type::Struct(_) => {
                        let length = ast.int_lit(self.typ.len() as i64);
                        let length_pre = ast.eq_cmp(ast.seq_length(arg_var), length);
                        let i = ast.new_var("i", ast.int_type());
                        let bound_pre = ast.forall(
                            &[i.0],
                            &[],
                            ast.implies(
                                ast.and(ast.le_cmp(ast.int_zero(), i.1), ast.lt_cmp(i.1, length)),
                                ctx.utils
                                    .bounded_f(ast.seq_index(arg_var, i.1), ctx.options.word_size),
                            ),
                        );
                        Some(ast.and(length_pre, bound_pre))
                    }
                    ir::Type::Int | ir::Type::Word => {
                        if is_predicate {
                            None
                        } else {
                            Some(ctx.utils.bounded_f(arg_var, ctx.options.word_size))
                        }
                    }
                    _ => None,
                }
            }
            EncodingMode::Bitvec => None,
        }
    }
}

impl<'a> ToViperType<'a> for ir::Type {
    fn to_viper_type(&self, ctx: &ViperEncodeCtx<'a>) -> viper::Type<'a> {
        let ast = ctx.ast;
        match self {
            ir::Type::Bool => ast.bool_type(),
            ir::Type::Int => ast.int_type(),
            ir::Type::Word => match ctx.encoding_mode {
                EncodingMode::Int => ast.int_type(),
                EncodingMode::Bitvec => ast.backend_bv64_type(),
            },
            ir::Type::Array => ctx.heap.get_type(),
            ir::Type::Struct(_) => ast.seq_type(ir::Type::Word.to_viper_type(ctx)),
            ir::Type::Ref => ast.ref_type(),
            ir::Type::Map(k, v) => ast.map_type(k.to_viper_type(ctx), v.to_viper_type(ctx)),
            ir::Type::Set(i) => ast.set_type(i.to_viper_type(ctx)),
            ir::Type::Seq(i) => ast.seq_type(i.to_viper_type(ctx)),
            x => panic!("Want type of {:?}", x),
        }
    }
}

impl FnDec {
    pub fn postcondition<'a>(&self, ctx: &ViperEncodeCtx<'a>) -> Option<Expr<'a>> {
        let ast = ctx.ast;
        match ctx.get_type(&self.retvar).unwrap() {
            Type::Int => {
                let retval = ast.local_var(&self.retvar, ast.int_type());
                Some(ctx.utils.bounded_f(retval, ctx.options.word_size))
            }
            Type::Word => match ctx.encoding_mode {
                EncodingMode::Int => {
                    let retval = ast.local_var(&self.retvar, ast.int_type());
                    Some(ctx.utils.bounded_f(retval, ctx.options.word_size))
                }
                EncodingMode::Bitvec => None,
            },
            struc @ Type::Struct(_) => match ctx.encoding_mode {
                EncodingMode::Int => {
                    // Length of the returned `IArray`
                    let retval = ast.local_var(&self.retvar, struc.to_viper_type(ctx));
                    let length = ast.int_lit(struc.len() as i64);
                    let length_post = ast.eq_cmp(ast.seq_length(retval), length);
                    // Bound of all elements
                    let i = ast.new_var("i", ast.int_type());
                    let bounded = ast.forall(
                        &[i.0],
                        &[],
                        ast.implies(
                            ast.and(ast.le_cmp(ast.int_zero(), i.1), ast.lt_cmp(i.1, length)),
                            ctx.utils
                                .bounded_f(ast.seq_index(retval, i.1), ctx.options.word_size),
                        ),
                    );

                    Some(ast.and(length_post, bounded))
                }
                EncodingMode::Bitvec => None,
            },
            _ => unreachable!(),
        }
    }
}

pub trait EncodingModeHelper<'a> {
    fn translate_op(
        &self,
        ast: AstFactory<'a>,
        optype: BinOpType,
        left: viper::Expr<'a>,
        right: viper::Expr<'a>,
    ) -> viper::Expr<'a>;
    fn translate_shift(
        &self,
        ast: AstFactory<'a>,
        shifttype: ShiftType,
        value: viper::Expr<'a>,
        shift_amount: viper::Expr<'a>,
    ) -> viper::Expr<'a>;
    fn to_bool(&self, ast: AstFactory<'a>, expr: viper::Expr<'a>) -> viper::Expr<'a>;
}

impl EncodingModeHelper<'_> for EncodingMode {
    fn translate_op<'a>(
        &self,
        ast: AstFactory<'a>,
        optype: BinOpType,
        left: viper::Expr<'a>,
        right: viper::Expr<'a>,
    ) -> viper::Expr<'a> {
        use ir::BinOpType::*;
        match self {
            EncodingMode::Int => match optype {
                Add => ast.add(left, right),
                Sub => ast.sub(left, right),
                Mul => ast.mul(left, right),
                Div => ast.div(left, right),
                Modulo => ast.module(left, right),
                Imp => ast.implies(left, right),
                Iff => ast.eq_cmp(left, right),
                BoolAnd => ast.and(left, right),
                BoolOr => ast.or(left, right),
                ViperNotEqual | PancakeNotEqual => ast.ne_cmp(left, right),
                ViperEqual | PancakeEqual => ast.eq_cmp(left, right),
                Lt | SignedLt => ast.lt_cmp(left, right),
                Lte | SignedLte => ast.le_cmp(left, right),
                Gt | SignedGt => ast.gt_cmp(left, right),
                Gte | SignedGte => ast.ge_cmp(left, right),
                BitAnd | BitOr | BitXor => {
                    let lbv = ast.int_to_backend_bv(BV64, left);
                    let rbv = ast.int_to_backend_bv(BV64, right);
                    ast.backend_bv_to_int(
                        BV64,
                        EncodingMode::Bitvec.translate_op(ast, optype, lbv, rbv),
                    )
                }
            },
            EncodingMode::Bitvec => match optype {
                Add => ast.bv_add(left, right),
                Sub => ast.bv_sub(left, right),
                Mul => ast.bv_mul(left, right),
                Div => ast.bv_div(left, right),
                Modulo => ast.bv_mod(left, right),
                Imp => ast.implies(left, right),
                Iff => ast.eq_cmp(left, right),
                BoolAnd => ast.and(left, right),
                BoolOr => ast.or(left, right),
                ViperNotEqual | PancakeNotEqual => ast.ne_cmp(left, right),
                ViperEqual | PancakeEqual => ast.eq_cmp(left, right),
                Lt => ast.bv_ult(left, right),
                SignedLt => ast.bv_slt(left, right),
                Lte => ast.bv_ule(left, right),
                SignedLte => ast.bv_sle(left, right),
                Gt => ast.bv_ugt(left, right),
                SignedGt => ast.bv_sgt(left, right),
                Gte => ast.bv_uge(left, right),
                SignedGte => ast.bv_sge(left, right),
                BitAnd => ast.bv_and(left, right),
                BitOr => ast.bv_or(left, right),
                BitXor => ast.bv_xor(left, right),
            },
        }
    }

    fn translate_shift<'a>(
        &self,
        ast: AstFactory<'a>,
        shifttype: ShiftType,
        value: viper::Expr<'a>,
        shift_amount: viper::Expr<'a>,
    ) -> viper::Expr<'a> {
        use ShiftType::*;
        match self {
            EncodingMode::Int => ast.backend_bv_to_int(
                BV64,
                EncodingMode::Bitvec.translate_shift(
                    ast,
                    shifttype,
                    ast.int_to_backend_bv(BV64, value),
                    ast.int_to_backend_bv(BV64, shift_amount),
                ),
            ),
            EncodingMode::Bitvec => match shifttype {
                Lsl => ast.bv_shl(value, shift_amount),
                Asr => ast.bv_ashr(value, shift_amount),
                Lsr => ast.bv_lshr(value, shift_amount),
            },
        }
    }

    fn to_bool<'a>(&self, ast: AstFactory<'a>, expr: viper::Expr<'a>) -> viper::Expr<'a> {
        ast.ne_cmp(expr, self.zero(ast))
    }
}
