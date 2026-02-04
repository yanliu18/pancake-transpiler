use crate::ir;
use viper::BvSize::BV64;

use crate::ir::shared::SharedOpType::{Load, Store};
use crate::ir_to_viper::utils::EncodingModeHelper;
use crate::utils::{
    EncodingMode, Shape, ToViperError, TryToShape, TryToViper, ViperEncodeCtx, ViperUtils,
};
use crate::viper_prelude::heap::MemType;

impl<'a> TryToViper<'a> for ir::Load {
    type Output = viper::Expr<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        let ast = ctx.ast;
        let encoder = ctx.encoding_mode;
        let bytes_in_word = encoder.lit(ctx.ast, ctx.options.word_size / 8);
        let heap = ctx.heap;
        let addr_exp = self.address.to_viper(ctx)?;
        let word_addr = match encoder {
            EncodingMode::Int => ast.div(addr_exp, bytes_in_word),
            EncodingMode::Bitvec => ast.bv_div(addr_exp, bytes_in_word),
        };

        if self.assert && ctx.options.assert_aligned_accesses {
            // assert addr % @biw == 0
            let comparison = match encoder {
                EncodingMode::Int => {
                    ast.eq_cmp(ast.module(addr_exp, bytes_in_word), ast.int_zero())
                }
                EncodingMode::Bitvec => {
                    ast.eq_cmp(ast.bv_mod(addr_exp, bytes_in_word), ast.bv_zero())
                }
            };
            let assertion = ast.assert(comparison, ast.no_position());
            ctx.stack.push(assertion);
        }

        let heap_var = ctx.utils.heap_var().1;
        Ok(if self.shape.is_simple() {
            heap.access(heap_var, word_addr, MemType::Local)
        } else {
            let length = self.shape.len();
            let elems = (0..length)
                .map(|offset| {
                    let sum = match encoder {
                        EncodingMode::Int => ast.add(word_addr, ast.int_lit(offset as i64)),
                        EncodingMode::Bitvec => {
                            ast.bv_add(word_addr, ast.backend_bv64_lit(offset as u64))
                        }
                    };
                    heap.access(heap_var, sum, MemType::Local)
                })
                .collect::<Vec<_>>();
            ast.explicit_seq(&elems)
        })
    }
}

impl<'a> TryToViper<'a> for ir::LoadBits {
    type Output = viper::Expr<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        let ast = ctx.ast;
        let encoder = ctx.encoding_mode;
        let bytes_in_word = encoder.lit(ast, ctx.options.word_size / 8);

        let byte_address = self.address.clone().to_viper(ctx)?;
        let word_offset = match encoder {
            EncodingMode::Int => ast.module(byte_address, bytes_in_word),
            EncodingMode::Bitvec => ast.bv_mod(byte_address, bytes_in_word),
        };
        let byte_mask = encoder.lit(ast, 2u64.pow(self.size.bits()) - 1);
        let shift_amount = match encoder {
            EncodingMode::Int => ast.mul(bytes_in_word, word_offset),
            EncodingMode::Bitvec => ast.bv_mul(bytes_in_word, word_offset),
        };

        let load = ir::Expr::Load(ir::Load {
            shape: Shape::Simple,
            address: self.address,
            assert: false,
        })
        .to_viper(ctx)?;

        match encoder {
            EncodingMode::Int => Ok(ast.backend_bv_to_int(
                BV64,
                ast.bv_and(
                    ast.int_to_backend_bv(BV64, byte_mask),
                    ast.bv_lshr(
                        ast.int_to_backend_bv(BV64, load),
                        ast.int_to_backend_bv(BV64, shift_amount),
                    ),
                ),
            )),
            EncodingMode::Bitvec => Ok(ast.bv_and(byte_mask, ast.bv_lshr(load, shift_amount))),
        }
    }
}

impl<'a> TryToViper<'a> for ir::Store {
    type Output = viper::Stmt<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        let ast = ctx.ast;
        let encoder = ctx.encoding_mode;
        let bytes_in_word = encoder.lit(ast, ctx.options.word_size / 8);
        let heap = ctx.heap;
        let addr_expr = self.address.to_viper(ctx)?;
        let word_addr = match encoder {
            EncodingMode::Int => ast.div(addr_expr, bytes_in_word),
            EncodingMode::Bitvec => ast.bv_div(addr_expr, bytes_in_word),
        };

        // assert addr % @biw == 0
        let assertion = if ctx.options.assert_aligned_accesses {
            let condition = match encoder {
                EncodingMode::Int => {
                    ast.eq_cmp(ast.module(addr_expr, bytes_in_word), ast.int_zero())
                }
                EncodingMode::Bitvec => {
                    ast.eq_cmp(ast.bv_mod(addr_expr, bytes_in_word), ast.bv_zero())
                }
            };
            ast.assert(condition, ast.no_position())
        } else {
            ast.comment("skipping alignment assertion")
        };

        let rhs_shape = self.value.to_shape(ctx.typectx_get_mut())?;
        let rhs = self.value.to_viper(ctx)?;
        let heap_var = ctx.utils.heap_var().1;

        let store = if rhs_shape.is_simple() {
            ast.field_assign(heap.access(heap_var, word_addr, MemType::Local), rhs)
        } else {
            let length = rhs_shape.len();

            let elems = (0..length)
                .map(|offset| {
                    let src = ast.seq_index(rhs, encoder.lit(ast, offset as u64));
                    let dst = ctx.heap.access(
                        heap_var,
                        match ctx.encoding_mode {
                            EncodingMode::Int => {
                                ast.add(word_addr, encoder.lit(ast, offset as u64))
                            }
                            EncodingMode::Bitvec => {
                                ast.bv_add(word_addr, encoder.lit(ast, offset as u64))
                            }
                        },
                        MemType::Local,
                    );
                    ast.field_assign(dst, src)
                })
                .collect::<Vec<_>>();
            ast.seqn(&elems, &[])
        };

        Ok(ast.seqn(&[assertion, store], &[]))
    }
}

impl<'a> TryToViper<'a> for ir::StoreBits {
    type Output = viper::Stmt<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        let ast = ctx.ast;
        let heap = ctx.heap;
        let encoder = ctx.encoding_mode;
        let bytes_in_word = encoder.lit(ast, ctx.options.word_size / 8);

        let assertion = if ctx.options.assert_aligned_accesses && self.size.bits() != 8 {
            let cond = match encoder {
                EncodingMode::Int => ast.eq_cmp(
                    ast.module(
                        self.address.clone().to_viper(ctx)?,
                        ast.int_lit(self.size.bytes() as i64),
                    ),
                    ast.int_zero(),
                ),
                EncodingMode::Bitvec => ast.eq_cmp(
                    ast.bv_mod(
                        self.address.clone().to_viper(ctx)?,
                        ast.backend_bv64_lit(self.size.bytes() as u64),
                    ),
                    ast.bv_zero(),
                ),
            };
            ast.assert(cond, ast.no_position())
        } else {
            ast.comment("skipping alignment assertion")
        };

        let byte_address = self.address.to_viper(ctx)?;
        let word_offset = match encoder {
            EncodingMode::Int => ast.module(byte_address, bytes_in_word),
            EncodingMode::Bitvec => ast.bv_mod(byte_address, bytes_in_word),
        };
        let word_index = match encoder {
            EncodingMode::Int => ast.div(byte_address, ast.int_lit(8)),
            EncodingMode::Bitvec => ast.bv_div(byte_address, ast.backend_bv64_lit(8)),
        };
        let bit_mask = ast.backend_bv64_lit(2_u64.pow(self.size.bits()) - 1);
        let shift_amount = match encoder {
            EncodingMode::Int => ast.int_to_backend_bv(BV64, ast.mul(bytes_in_word, word_offset)),
            EncodingMode::Bitvec => ast.bv_mul(bytes_in_word, word_offset),
        };
        let mask = ast.bv_shl(bit_mask, shift_amount);
        let inv_mask = ast.bv_not(mask);
        let value = self.value.to_viper(ctx)?;
        let value = match encoder {
            EncodingMode::Int => ast.int_to_backend_bv(BV64, value),
            EncodingMode::Bitvec => value,
        };
        let value = ast.bv_shl(ast.bv_and(bit_mask, value), shift_amount);
        let heap_var = ctx.utils.heap_var().1;
        let old = heap.access(heap_var, word_index, MemType::Local);
        let old = match encoder {
            EncodingMode::Int => ast.int_to_backend_bv(BV64, old),
            EncodingMode::Bitvec => old,
        };
        let new = ast.bv_or(ast.bv_and(old, inv_mask), ast.bv_and(value, mask));
        let new = match encoder {
            EncodingMode::Int => ast.backend_bv_to_int(BV64, new),
            EncodingMode::Bitvec => new,
        };
        let field_ass = ast.field_assign(heap.access(heap_var, word_index, MemType::Local), new);
        Ok(ast.seqn(&[assertion, field_ass], &[]))
    }
}

impl<'a> TryToViper<'a> for ir::SharedStore {
    type Output = viper::Stmt<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        ir::SharedStoreBits {
            address: self.address,
            value: self.value,
            size: ctx.options.word_size.into(),
        }
        .to_viper(ctx)
    }
}

impl<'a> TryToViper<'a> for ir::SharedStoreBits {
    type Output = viper::Stmt<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        let ast = ctx.ast;
        let encoder = ctx.encoding_mode;
        let addr_expr = self.address.clone().to_viper(ctx)?;
        // assert correct alignment
        let assertion = if ctx.options.assert_aligned_accesses && self.size.bytes() != 1 {
            let cond = match encoder {
                EncodingMode::Bitvec => ast.eq_cmp(
                    ast.bv_mod(addr_expr, encoder.lit(ast, self.size.bytes().into())),
                    ast.bv_zero(),
                ),
                EncodingMode::Int => ast.eq_cmp(
                    ast.module(addr_expr, encoder.lit(ast, self.size.bytes().into())),
                    ast.int_zero(),
                ),
            };
            ast.assert(cond, ast.no_position())
        } else {
            ast.comment("skipping alignment assertion")
        };
        let value = self.value.to_viper(ctx)?;
        let mut args = ctx.get_default_args().1;
        args.push(addr_expr);
        args.push(value);
        if let Some(name) = &ctx.shared_override {
            let call = ast.seqn(
                &[
                    assertion,
                    ast.method_call(&format!("store_{}", name), &args, &[]),
                ],
                &[],
            );
            ctx.shared_override = None;
            return Ok(call);
        }
        match self.address {
            ir::Expr::Const(addr) => {
                let store_stmt = ast.method_call(
                    &ctx.shared
                        .get_method_name(addr, ctx.options, Store, self.size),
                    &args,
                    &[],
                );
                Ok(ast.seqn(&[assertion, store_stmt], &[]))
            }
            _ => Ok(ctx
                .shared
                .get_switch(ctx, addr_expr, Store, self.size, value)),
        }
    }
}

impl<'a> TryToViper<'a> for ir::SharedLoad {
    type Output = viper::Stmt<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        ir::SharedLoadBits {
            address: self.address,
            dst: self.dst,
            size: ctx.options.word_size.into(),
        }
        .to_viper(ctx)
    }
}

impl<'a> TryToViper<'a> for ir::SharedLoadBits {
    type Output = viper::Stmt<'a>;
    fn to_viper(self, ctx: &mut ViperEncodeCtx<'a>) -> Result<Self::Output, ToViperError> {
        let ast = ctx.ast;
        let encoder = ctx.encoding_mode;
        let addr_expr = self.address.clone().to_viper(ctx)?;
        // assert correct alignment
        let assertion = if ctx.options.assert_aligned_accesses && self.size.bits() != 8 {
            let cond = match encoder {
                EncodingMode::Bitvec => ast.eq_cmp(
                    ast.bv_mod(addr_expr, encoder.lit(ast, self.size.bytes().into())),
                    ast.bv_zero(),
                ),
                EncodingMode::Int => ast.eq_cmp(
                    ast.module(addr_expr, encoder.lit(ast, self.size.bytes().into())),
                    ast.int_zero(),
                ),
            };
            ast.assert(cond, ast.no_position())
        } else {
            ast.comment("skipping alignment assertion")
        };
        let dst = self.dst.to_viper(ctx)?;
        let mut args = ctx.get_default_args().1;
        args.push(addr_expr);
        if let Some(name) = &ctx.shared_override {
            let call = ast.seqn(
                &[
                    assertion,
                    ast.method_call(&format!("load_{}", name), &args, &[dst]),
                ],
                &[],
            );
            ctx.shared_override = None;
            return Ok(call);
        }
        match &self.address {
            ir::Expr::Const(addr) => {
                let store_stmt = ast.method_call(
                    &ctx.shared
                        .get_method_name(*addr, ctx.options, Load, self.size),
                    &args,
                    &[dst],
                );
                Ok(ast.seqn(&[assertion, store_stmt], &[]))
            }
            _ => Ok(ctx.shared.get_switch(ctx, addr_expr, Load, self.size, dst)),
        }
    }
}
